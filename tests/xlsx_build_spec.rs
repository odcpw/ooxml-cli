use ooxml_cli::build::{BuildFamily, compile_xlsx_spec, load_spec_file, load_spec_str};
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const SALES_SPEC: &str = "testdata/xlsx/build-spec/sales.json";
const SALES_GOLDEN: &str = "testdata/golden/build-spec/xlsx/sales-summary.json";

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

fn assert_success(output: &Output, label: &str) {
    assert!(
        output.status.success(),
        "{label} failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn temp_dir(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "ooxml-xlsx-build-spec-{label}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create test directory");
    path
}

#[test]
fn sales_recipe_compiles_to_the_expected_atomic_operation_sequence() {
    let spec = load_spec_file(BuildFamily::Xlsx, SALES_SPEC).expect("load committed sales spec");
    let compiled = compile_xlsx_spec(&spec).expect("compile committed sales spec");
    let commands = compiled
        .plan
        .operations
        .iter()
        .map(|operation| operation.command.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        commands,
        [
            "xlsx scaffold",
            "xlsx ranges set",
            "xlsx sheets set-tab-color",
            "xlsx freeze set",
            "xlsx ranges set-style",
            "xlsx colwidths set",
            "xlsx colwidths set",
            "xlsx colwidths set",
            "xlsx ranges set-format",
            "xlsx colwidths set",
            "xlsx ranges set-format",
            "xlsx colwidths set",
            "xlsx ranges set-format",
            "xlsx tables create",
            "xlsx conditional-formats add",
            "xlsx data-validations create",
            "xlsx names add",
            "xlsx charts create",
            "xlsx hyperlinks add",
            "xlsx comments add",
            "xlsx sheets set-print",
            "xlsx ranges set",
            "xlsx sheets set-tab-color",
            "xlsx freeze set",
            "xlsx ranges set-style",
            "xlsx colwidths autofit",
            "xlsx colwidths autofit",
            "xlsx colwidths set",
            "xlsx ranges set-format",
            "xlsx colwidths set",
            "xlsx colwidths set",
            "xlsx ranges set-format",
            "xlsx colwidths set",
            "xlsx ranges set-format",
            "xlsx tables create",
            "xlsx workbook metadata update",
        ]
    );
    assert_eq!(
        compiled.plan.operations[1].args["values"]
            .as_str()
            .and_then(|values| serde_json::from_str::<Value>(values).ok())
            .and_then(|values| values.pointer("/1/3").cloned()),
        Some(json!({"formula": "B2*C2"}))
    );
    assert_eq!(
        compiled.plan.operations[13].args["range"],
        json!({"$ref": "sheet-1-data.destination.range"})
    );
    assert_eq!(compiled.plan.operations[28].args["range"], "C:C");
    assert_eq!(compiled.plan.operations[35].args["fullCalcOnLoad"], true);
}

#[test]
fn sales_recipe_builds_strict_sdk_clean_renderable_deterministic_workbook() {
    let temp = temp_dir("sales");
    let first = temp.join("sales-first.xlsx");
    let second = temp.join("sales-second.xlsx");
    let first_path = first.to_str().expect("first output path");
    let second_path = second.to_str().expect("second output path");

    let built = run(&[
        "--json", "xlsx", "build", "--spec", SALES_SPEC, "--out", first_path, "--check",
    ]);
    assert_success(&built, "sales workbook build");
    let result = json_stdout(&built);
    assert_eq!(result["validated"], true);
    assert_eq!(result["mutationEnvelope"]["opsCount"], 36);
    assert_eq!(result["check"]["summary"]["errors"], 0);
    assert_golden(&semantic_summary(&result));

    let strict = run(&["--json", "validate", "--strict", first_path]);
    assert_success(&strict, "strict validation");
    assert_openxml_sdk_clean(&first);

    let cells = run(&[
        "--json",
        "xlsx",
        "ranges",
        "export",
        first_path,
        "--sheet",
        "Sales",
        "--range",
        "A1:E5",
        "--include-types",
        "--include-formulas",
        "--include-formats",
    ]);
    assert_success(&cells, "sales range export");
    let cells = json_stdout(&cells);
    assert_eq!(cells["formulas"][1][3], "B2*C2");
    assert_eq!(cells["types"][1][1], "number");
    assert_eq!(cells["numberFormatCodes"][1][2], "$#,##0.00");
    assert_eq!(cells["numberFormatCodes"][1][4], "0.0%");

    let targets = run(&[
        "--json",
        "xlsx",
        "ranges",
        "export",
        first_path,
        "--sheet",
        "Targets",
        "--range",
        "C2:F2",
        "--include-types",
        "--include-formats",
    ]);
    assert_success(&targets, "typed external range export");
    let targets = json_stdout(&targets);
    assert_eq!(targets["values"], json!([[1800.0, true, "46037.0", 0.9]]));
    assert_eq!(
        targets["types"],
        json!([["number", "boolean", "date", "number"]])
    );
    assert_eq!(targets["numberFormatCodes"][0][0], "$#,##0");
    assert_eq!(targets["numberFormatCodes"][0][2], "yyyy-mm-dd");
    assert_eq!(targets["numberFormatCodes"][0][3], "0%");

    let tables = run(&["--json", "xlsx", "tables", "list", first_path]);
    assert_success(&tables, "table readback");
    assert_eq!(
        json_stdout(&tables)["tables"].as_array().map(Vec::len),
        Some(2)
    );
    let charts = run(&["--json", "xlsx", "charts", "list", first_path]);
    assert_success(&charts, "chart readback");
    assert_eq!(
        json_stdout(&charts)["charts"].as_array().map(Vec::len),
        Some(1)
    );
    let metadata = run(&[
        "--json", "xlsx", "workbook", "metadata", "inspect", first_path,
    ]);
    assert_success(&metadata, "metadata readback");
    let metadata = json_stdout(&metadata);
    assert_eq!(metadata["metadata"]["title"], "Quarterly Sales Workbook");
    assert_eq!(metadata["calcSettings"]["fullCalcOnLoad"], true);
    assert_eq!(metadata["calcSettings"]["forceFullCalc"], true);

    assert_libreoffice_render(&temp, first_path);

    let second_build = run(&[
        "--json",
        "xlsx",
        "build",
        "--spec",
        SALES_SPEC,
        "--out",
        second_path,
    ]);
    assert_success(&second_build, "second sales workbook build");
    assert_eq!(
        fs::read(&first).expect("read first workbook"),
        fs::read(&second).expect("read second workbook"),
        "identical specs must produce byte-identical workbooks"
    );
    let refused_replace = run(&[
        "--json", "xlsx", "build", "--spec", SALES_SPEC, "--out", first_path,
    ]);
    assert_eq!(refused_replace.status.code(), Some(2));
    let refused_message = if refused_replace.stdout.is_empty() {
        String::from_utf8_lossy(&refused_replace.stderr)
    } else {
        String::from_utf8_lossy(&refused_replace.stdout)
    };
    assert!(
        refused_message.contains("output file already exists; pass --force"),
        "unexpected replacement refusal: {refused_message}"
    );
    let forced_replace = run(&[
        "--json", "xlsx", "build", "--spec", SALES_SPEC, "--out", first_path, "--force",
    ]);
    assert_success(&forced_replace, "forced deterministic replacement");
    assert_eq!(
        fs::read(&first).expect("read forced replacement"),
        fs::read(&second).expect("reread second workbook")
    );
    let _ = fs::remove_dir_all(temp);
}

#[test]
fn dry_run_validates_the_complete_batch_without_publishing() {
    let temp = temp_dir("dry-run");
    let output = temp.join("planned.xlsx");
    let run = run(&[
        "--json",
        "xlsx",
        "build",
        "--spec",
        SALES_SPEC,
        "--out",
        output.to_str().expect("output path"),
        "--dry-run",
    ]);
    assert_success(&run, "XLSX build dry-run");
    let result = json_stdout(&run);
    assert_eq!(result["dryRun"], true);
    assert_eq!(result["validated"], true);
    assert_eq!(result["mutationEnvelope"]["committed"], false);
    assert_eq!(result["mutationEnvelope"]["opsCount"], 36);
    assert_eq!(result["outline"], Value::Null);
    assert!(!output.exists());
    let serialized = serde_json::to_string(&result).expect("serialize dry-run response");
    assert!(serialized.contains("<build-stage>"));
    assert!(serialized.contains("targets.csv"));
    assert!(
        !serialized.contains(
            &std::env::current_dir()
                .expect("current directory")
                .to_string_lossy()
                .to_string()
        )
    );
    assert!(!serialized.contains(&format!("ooxml-xlsx-build-{}", std::process::id())));
    let _ = fs::remove_dir_all(temp);
}

#[test]
fn compiler_refuses_ambiguous_sources_and_invalid_typed_values() {
    let ambiguous = load_spec_str(
        BuildFamily::Xlsx,
        &json!({
            "schemaVersion": 1,
            "family": "xlsx",
            "sheets": [{
                "name": "Data",
                "rows": [["Value"], [1]],
                "dataFile": {"path": "data.csv", "format": "csv"},
                "columns": [{"name": "Value", "type": "number"}]
            }]
        })
        .to_string(),
    )
    .expect("schema permits each source form independently");
    let error = compile_xlsx_spec(&ambiguous).expect_err("ambiguous data sources must fail");
    assert_eq!(error.code, "BUILD_SPEC_VALUE_INVALID");
    assert_eq!(error.path, "/sheets/0");
    assert!(error.message.contains("only one sheet data source"));

    let invalid_typed = load_spec_str(
        BuildFamily::Xlsx,
        &json!({
            "schemaVersion": 1,
            "family": "xlsx",
            "sheets": [{
                "name": "Data",
                "rows": [["Amount"], ["not-money"]],
                "columns": [{"name": "Amount", "type": "currency"}]
            }]
        })
        .to_string(),
    )
    .expect("typed value validation belongs to the compiler");
    let error = compile_xlsx_spec(&invalid_typed).expect_err("invalid currency must fail");
    assert_eq!(error.path, "/sheets/0/rows/1/0");
    assert!(error.message.contains("invalid currency value"));
}

fn semantic_summary(result: &Value) -> Value {
    let sheets = result["outline"]["sheets"]
        .as_array()
        .expect("outline sheets")
        .iter()
        .map(|sheet| {
            json!({
                "name": sheet["name"],
                "rowCount": sheet["rowCount"],
                "cellCount": sheet["cellCount"],
                "usedRange": sheet["usedRange"]["ref"],
                "freeze": sheet["freeze"],
                "commentCount": sheet["commentCount"],
                "conditionalFormatCount": sheet["conditionalFormatCount"],
                "validationCount": sheet["validationCount"],
            })
        })
        .collect::<Vec<_>>();
    let node_ids = result["compiledPlan"]["nodeMap"]
        .as_object()
        .expect("compiled node map")
        .values()
        .filter_map(|node| node["specId"].as_str())
        .collect::<Vec<_>>();
    let operations = result["compiledPlan"]["operations"]
        .as_array()
        .expect("compiled operations")
        .iter()
        .map(|operation| operation["command"].clone())
        .collect::<Vec<_>>();
    json!({
        "schemaVersion": result["schemaVersion"],
        "validated": result["validated"],
        "checkErrors": result["check"]["summary"]["errors"],
        "nodeIds": node_ids,
        "operations": operations,
        "sheets": sheets,
        "summary": result["outline"]["summary"],
    })
}

fn assert_golden(actual: &Value) {
    let mut rendered = serde_json::to_string_pretty(actual).expect("serialize sales summary");
    rendered.push('\n');
    if std::env::var("UPDATE_GOLDENS").as_deref() == Ok("1") {
        fs::create_dir_all(Path::new(SALES_GOLDEN).parent().expect("golden parent"))
            .expect("create golden directory");
        fs::write(SALES_GOLDEN, &rendered).expect("update reviewed sales summary golden");
    }
    let expected = fs::read_to_string(SALES_GOLDEN).unwrap_or_else(|error| {
        panic!("missing {SALES_GOLDEN}: {error}; rerun with UPDATE_GOLDENS=1")
    });
    assert_eq!(rendered, expected, "sales build semantic summary drifted");
}

fn assert_openxml_sdk_clean(workbook: &Path) {
    let output = run(&[
        "--json",
        "conformance",
        "check",
        workbook.to_str().expect("workbook path"),
        "--openxml-sdk",
    ]);
    let report = json_stdout(&output);
    let schema = report["checks"]
        .as_array()
        .expect("conformance checks")
        .iter()
        .find(|check| check["name"] == "schema")
        .expect("Open XML SDK schema check");
    if schema["status"] == "skipped" {
        let required = std::env::var("OOXML_REQUIRE_OPENXML_SDK")
            .is_ok_and(|value| value == "1" || value.eq_ignore_ascii_case("true"));
        assert!(
            !required,
            "Open XML SDK proof was required but skipped; run `ooxml --json doctor --only openxml-sdk`: {schema}"
        );
        assert_eq!(schema["schemaCheck"]["checked"], false);
        assert!(schema["diagnostics"][0].get("remediation").is_some());
        return;
    }
    assert_success(&output, "Open XML SDK validation");
    assert_eq!(schema["status"], "passed");
    assert_eq!(schema["schemaCheck"]["checked"], true);
    assert_eq!(schema["schemaCheck"]["valid"], true);
    assert_eq!(schema["schemaCheck"]["errorCount"], 0);
}

fn assert_libreoffice_render(temp: &Path, workbook: &str) {
    let render_dir = temp.join("render");
    let render = run(&[
        "--json",
        "render",
        workbook,
        "--out",
        render_dir.to_str().expect("render path"),
        "--sheet",
        "Sales",
    ]);
    assert_success(&render, "LibreOffice render");
    let result = json_stdout(&render);
    if result["status"] == "skipped" {
        assert_eq!(
            result["doctorCommand"],
            "ooxml --json doctor --only render-engine,fonts"
        );
        return;
    }
    assert_eq!(result["status"], "ok");
    assert_eq!(result["engine"], "libreoffice");
    let pdf = Path::new(result["pdfPath"].as_str().expect("render PDF"));
    assert!(pdf.is_file());
    assert!(
        !result["pages"]
            .as_array()
            .expect("rendered pages")
            .is_empty()
    );
    for page in result["pages"].as_array().expect("rendered pages") {
        assert!(Path::new(page["imagePath"].as_str().expect("page image")).is_file());
    }
    if Command::new("pdftotext").arg("-v").output().is_ok() {
        let text = Command::new("pdftotext")
            .arg("-layout")
            .arg(pdf)
            .arg("-")
            .output()
            .expect("extract LibreOffice-rendered PDF text");
        assert_success(&text, "PDF text extraction");
        let text = String::from_utf8_lossy(&text.stdout);
        for expected in ["Unit Price", "$1,506.00", "25.0%", "Total", "$5,999.25"] {
            assert!(
                text.contains(expected),
                "LibreOffice render omitted formatted value {expected:?}: {text}"
            );
        }
    }
}
