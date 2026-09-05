use ooxml_cli::build::{BuildFamily, compile_xlsx_spec, load_spec_bytes, markdown_to_spec};
use serde_json::{Value, json};

const SOURCE: &str = include_str!("../testdata/markdown/mapping-xlsx.md");

#[test]
fn workbook_conversion_maps_sections_types_totals_and_chart_to_existing_operations() {
    let converted = markdown_to_spec(BuildFamily::Xlsx, SOURCE, "mapping-xlsx.md").unwrap();
    assert!(converted.warnings.is_empty(), "{:?}", converted.warnings);
    let sheets = converted.spec["sheets"].as_array().unwrap();
    assert_eq!(sheets.len(), 2);
    assert_eq!(sheets[0]["name"], "Sales");
    assert_eq!(sheets[1]["name"], "Targets");
    let kinds = sheets[0]["columns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|column| column["type"].clone())
        .collect::<Vec<_>>();
    assert_eq!(
        json!(kinds),
        json!([
            "text", "number", "currency", "percent", "boolean", "date", "text"
        ])
    );
    assert_eq!(sheets[0]["rows"][0][2], "Revenue");
    assert_eq!(sheets[0]["rows"].as_array().unwrap().len(), 3);
    assert_eq!(
        sheets[0]["tables"][0]["totals"],
        json!(["Units:sum", "Revenue:sum"])
    );
    assert_eq!(sheets[0]["freeze"], "A2");
    let spec = load_spec_bytes(
        BuildFamily::Xlsx,
        &serde_json::to_vec(&converted.spec).unwrap(),
    )
    .unwrap();
    let plan = serde_json::to_value(compile_xlsx_spec(&spec).unwrap()).unwrap();
    let operations = plan["plan"]["operations"].as_array().unwrap();
    let data = operations
        .iter()
        .find(|op| op["command"] == "xlsx ranges set")
        .unwrap();
    let rows: Value = serde_json::from_str(data["args"]["values"].as_str().unwrap()).unwrap();
    assert_eq!(rows[1][1], 12.0);
    assert_eq!(rows[1][2], 1506.0);
    assert_eq!(rows[1][3], 0.25);
    assert_eq!(rows[1][4], true);
    assert_eq!(rows[1][5], 46037.0);
    assert_eq!(rows[1][6], "001");
    assert!(
        operations
            .iter()
            .any(|op| op["command"] == "xlsx charts create")
    );
}

#[test]
fn workbook_conversion_refuses_ambiguous_sections_and_invalid_hints() {
    for source in [
        "# Data\n| X |\n| --- |\n| 1 |\n\n| Y |\n| --- |\n| 2 |\n",
        "# Bad/Name\n| X |\n| --- |\n| 1 |\n",
        "# Data\n| X (number) |\n| --- |\n| nope |\n",
        "# Data\n| X (date) |\n| --- |\n| 2026-02-30 |\n",
        "# Data\n| X |\n| --- |\n| Total |\n| 2 |\n",
        "# Data\n```chart\n{}\n```\n",
        "# Data\n| X | X (text) |\n| --- | --- |\n| 1 | 2 |\n",
    ] {
        assert!(
            markdown_to_spec(BuildFamily::Xlsx, source, "invalid.md").is_err(),
            "accepted {source}"
        );
    }
}

#[test]
fn workbook_mixed_columns_remain_text_and_chart_defaults_to_section_table() {
    let converted = markdown_to_spec(BuildFamily::Xlsx, "| Mixed | Empty |\n| --- | --- |\n| 1 | |\n| two | |\n\n```chart\n{\"type\":\"bar\"}\n```\n", "table.md").unwrap();
    let sheet = &converted.spec["sheets"][0];
    assert_eq!(sheet["columns"][0]["type"], "text");
    assert_eq!(sheet["columns"][1]["type"], "text");
    assert_eq!(sheet["rows"][1][1], Value::Null);
    assert_eq!(sheet["charts"][0]["options"]["table"], "MarkdownTable1");
}

fn run(args: &[&str]) -> Value {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{args:?}: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn golden(path: &str, value: &Value) {
    let bytes = format!("{}\n", serde_json::to_string_pretty(value).unwrap());
    if std::env::var("UPDATE_GOLDENS").as_deref() == Ok("1") {
        std::fs::write(path, &bytes).unwrap();
    }
    assert_eq!(std::fs::read_to_string(path).unwrap(), bytes, "{path}");
}

#[test]
fn workbook_fixture_builds_deterministically_with_strict_and_sdk_proof() {
    let converted = markdown_to_spec(BuildFamily::Xlsx, SOURCE, "mapping-xlsx.md").unwrap();
    golden(
        "testdata/golden/markdown-workbook-spec.json",
        &converted.spec,
    );
    let dir = std::env::temp_dir().join(format!(
        "ooxml-markdown-workbook-proof-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let spec = dir.join("spec.json");
    std::fs::write(&spec, serde_json::to_vec(&converted.spec).unwrap()).unwrap();
    let first = dir.join("first.xlsx");
    let second = dir.join("second.xlsx");
    for path in [&first, &second] {
        let result = run(&[
            "--json",
            "xlsx",
            "build",
            "--spec",
            spec.to_str().unwrap(),
            "--out",
            path.to_str().unwrap(),
            "--force",
        ]);
        assert_eq!(result["validated"], true);
        let strict = run(&["--json", "validate", "--strict", path.to_str().unwrap()]);
        assert_eq!(strict["valid"], true);
    }
    assert_eq!(
        std::fs::read(&first).unwrap(),
        std::fs::read(&second).unwrap()
    );
    let package_golden = "testdata/xlsx/markdown/mapping.xlsx";
    let package_bytes = std::fs::read(&first).unwrap();
    if std::env::var("UPDATE_GOLDENS").as_deref() == Ok("1") {
        std::fs::create_dir_all(std::path::Path::new(package_golden).parent().unwrap()).unwrap();
        std::fs::write(package_golden, &package_bytes).unwrap();
    }
    assert_eq!(
        std::fs::read(package_golden).unwrap(),
        package_bytes,
        "committed workbook bytes drifted"
    );
    let outline = run(&["--json", "outline", first.to_str().unwrap(), "--depth", "3"]);
    golden(
        "testdata/golden/markdown-workbook-outline.json",
        &json!({"type": outline["type"], "summary": outline["summary"], "sheets": outline["sheets"]}),
    );
    let cells = run(&[
        "--json",
        "xlsx",
        "ranges",
        "export",
        first.to_str().unwrap(),
        "--sheet",
        "Sales",
        "--range",
        "A1:G4",
        "--include-types",
    ]);
    assert_eq!(cells["values"][1][1], 12.0);
    assert_eq!(cells["values"][1][2], 1506.0);
    assert_eq!(cells["values"][1][3], 0.25);
    assert_eq!(cells["values"][1][4], true);
    assert_eq!(cells["values"][1][6], "001");
    assert_eq!(cells["values"][3][0], "Total");
    let schema = run(&[
        "--json",
        "conformance",
        "check",
        first.to_str().unwrap(),
        "--openxml-sdk",
    ]);
    let schema = schema["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["name"] == "schema")
        .unwrap();
    eprintln!("Markdown workbook SDK status: {}", schema["status"]);
    if schema["status"] == "skipped" {
        assert!(
            !std::env::var("OOXML_REQUIRE_OPENXML_SDK")
                .is_ok_and(|value| value == "1" || value.eq_ignore_ascii_case("true")),
            "SDK required: {schema}"
        );
    } else {
        assert_eq!(schema["status"], "passed", "{schema}");
        assert_eq!(schema["schemaCheck"]["errorCount"], 0);
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn workbook_markdown_cli_preserves_lexical_paths_emits_spec_and_matches_json_build() {
    let dir = std::env::temp_dir().join(format!(
        "ooxml-markdown-workbook-cli-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let input = format!("{}/./input.md", dir.display());
    let output = format!("{}/./output.xlsx", dir.display());
    let emitted = format!("{}/./emitted.json", dir.display());
    std::fs::write(&input, SOURCE).unwrap();
    let result = run(&[
        "--json",
        "xlsx",
        "build",
        "--from-markdown",
        &input,
        "--emit-spec",
        &emitted,
        "--out",
        &output,
        "--force",
    ]);
    assert_eq!(result["markdown"], input);
    assert_eq!(result["output"], output);
    assert_eq!(result["emittedSpec"], emitted);
    assert_eq!(result["spec"], Value::Null);
    assert_eq!(result["validated"], true);
    let conversion = markdown_to_spec(BuildFamily::Xlsx, SOURCE, &input).unwrap();
    let spec: Value = serde_json::from_slice(&std::fs::read(&emitted).unwrap()).unwrap();
    assert_eq!(spec, conversion.spec);
    let twin = dir.join("twin.xlsx");
    run(&[
        "--json",
        "xlsx",
        "build",
        "--spec",
        &emitted,
        "--out",
        twin.to_str().unwrap(),
        "--force",
    ]);
    assert_eq!(
        std::fs::read(&output).unwrap(),
        std::fs::read(&twin).unwrap()
    );
    let relative = std::process::Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .current_dir(&dir)
        .args([
            "--json",
            "xlsx",
            "build",
            "--from-markdown",
            "input.md",
            "--out",
            "relative.xlsx",
        ])
        .output()
        .unwrap();
    assert!(
        relative.status.success(),
        "{}",
        String::from_utf8_lossy(&relative.stdout)
    );
    let relative: Value = serde_json::from_slice(&relative.stdout).unwrap();
    assert_eq!(relative["markdown"], "input.md");
    assert_eq!(relative["output"], "relative.xlsx");
    assert_eq!(
        std::fs::read(dir.join("relative.xlsx")).unwrap(),
        std::fs::read(&output).unwrap()
    );
    let dry_output = dir.join("dry.xlsx");
    let dry = run(&[
        "--json",
        "xlsx",
        "build",
        "--from-markdown",
        &input,
        "--out",
        dry_output.to_str().unwrap(),
        "--dry-run",
    ]);
    assert_eq!(dry["dryRun"], true);
    assert!(!dry_output.exists());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn workbook_markdown_cli_stdin_and_invalid_input_preserve_existing_output() {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let dir = std::env::temp_dir().join(format!(
        "ooxml-markdown-workbook-stdin-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let output = dir.join("stdin.xlsx");
    let mut child = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args([
            "--json",
            "xlsx",
            "build",
            "--from-markdown",
            "-",
            "--out",
            output.to_str().unwrap(),
            "--force",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(SOURCE.as_bytes())
        .unwrap();
    let response = child.wait_with_output().unwrap();
    assert!(
        response.status.success(),
        "{}",
        String::from_utf8_lossy(&response.stderr)
    );
    let result: Value = serde_json::from_slice(&response.stdout).unwrap();
    assert_eq!(result["markdown"], "-");
    let prior = std::fs::read(&output).unwrap();
    let invalid = dir.join("invalid.md");
    std::fs::write(
        &invalid,
        "# Data\n| Value (number) |\n| --- |\n| invalid |\n",
    )
    .unwrap();
    for source_args in [
        vec!["--from-markdown", invalid.to_str().unwrap()],
        vec![
            "--from-markdown",
            invalid.to_str().unwrap(),
            "--spec",
            "missing.json",
        ],
        vec!["--emit-spec", "unused.json", "--spec", "missing.json"],
    ] {
        let failure = Command::new(env!("CARGO_BIN_EXE_ooxml"))
            .args([
                "--json",
                "xlsx",
                "build",
                "--out",
                output.to_str().unwrap(),
                "--force",
            ])
            .args(source_args)
            .output()
            .unwrap();
        assert_eq!(failure.status.code(), Some(2));
        assert_eq!(std::fs::read(&output).unwrap(), prior);
    }
    std::fs::remove_dir_all(dir).unwrap();
}
