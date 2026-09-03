use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

const GOLDEN: &str = "testdata/golden/mcp/typed-tools.json";
const TYPED_NAMES: [&str; 10] = [
    "build_presentation",
    "build_workbook",
    "build_document",
    "edit_package",
    "outline_package",
    "check_package",
    "validate_package",
    "render_preview",
    "find_text",
    "replace_text",
];

#[test]
fn tools_list_pins_typed_schemas_and_matches_cli_contracts() {
    let response = mcp(&[rpc(1, "tools/list", json!({}))], &[]).remove(0);
    let tools = response["result"]["tools"]
        .as_array()
        .expect("MCP tools array");
    let generic = tools
        .iter()
        .take_while(|tool| !TYPED_NAMES.contains(&tool["name"].as_str().unwrap_or_default()))
        .map(|tool| tool["name"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        generic,
        [
            "open", "op", "inspect", "validate", "plan", "commit", "abort"
        ]
    );

    let typed = tools
        .iter()
        .filter(|tool| TYPED_NAMES.contains(&tool["name"].as_str().unwrap_or_default()))
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(typed.len(), TYPED_NAMES.len());
    assert_eq!(
        typed
            .iter()
            .map(|tool| tool["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        TYPED_NAMES
    );
    for tool in &typed {
        assert_eq!(tool["inputSchema"]["type"], "object");
        assert_eq!(tool["inputSchema"]["additionalProperties"], false);
        assert!(
            !tool["description"].as_str().unwrap_or_default().is_empty(),
            "{} needs an agent-facing recipe hint",
            tool["name"]
        );
    }

    let capabilities = run_cli_json(&strings(&["--json", "capabilities"]), &[]);
    let command_contracts = capabilities["commands"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|command| command["opCompatible"] == true)
        .map(|command| {
            (
                command["path"]
                    .as_str()
                    .unwrap()
                    .trim_start_matches("ooxml ")
                    .to_string(),
                command["opArgsSchema"].clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let edit = typed
        .iter()
        .find(|tool| tool["name"] == "edit_package")
        .unwrap();
    let variants = edit["inputSchema"]["properties"]["operations"]["items"]["oneOf"]
        .as_array()
        .expect("manifest-derived operation variants");
    assert_eq!(variants.len(), command_contracts.len());
    for variant in variants {
        let command = variant["properties"]["command"]["const"].as_str().unwrap();
        assert_eq!(
            &variant["properties"]["args"],
            command_contracts
                .get(command)
                .expect("CLI command contract"),
            "typed edit args drifted from ooxml {command}"
        );
    }

    for (tool_name, schema_name) in [
        ("build_presentation", "pptx-build"),
        ("build_workbook", "xlsx-build"),
        ("build_document", "docx-build"),
    ] {
        let tool = typed.iter().find(|tool| tool["name"] == tool_name).unwrap();
        let schema = run_cli_json(
            &strings(&["--json", "capabilities", "--schema", schema_name]),
            &[],
        );
        assert_eq!(tool["inputSchema"]["properties"]["spec"], schema);
    }

    let actual = format!(
        "{}\n",
        serde_json::to_string_pretty(&typed).expect("serialize typed tools golden")
    );
    if std::env::var_os("UPDATE_GOLDENS").is_some() {
        std::fs::create_dir_all(Path::new(GOLDEN).parent().unwrap()).unwrap();
        std::fs::write(GOLDEN, actual.as_bytes()).expect("write reviewed typed tools golden");
    }
    assert_eq!(
        actual,
        std::fs::read_to_string(GOLDEN).expect("typed tools golden")
    );
}

#[test]
fn typed_build_tools_create_three_strictly_valid_recipe_outputs() {
    let dir = temp_dir("build");
    let pptx = dir.join("typed.pptx");
    let xlsx = dir.join("typed.xlsx");
    let docx = dir.join("typed.docx");
    let requests = vec![
        tool_call(
            1,
            "build_presentation",
            json!({
                "output": pptx,
                "spec": {"schemaVersion": 1, "family": "pptx", "slides": [{"layout": "Title Slide", "title": "Typed MCP", "subtitle": "One call"}]},
            }),
        ),
        tool_call(
            2,
            "build_workbook",
            json!({
                "output": xlsx,
                "spec": {"schemaVersion": 1, "family": "xlsx", "sheets": [{"name": "Data"}]},
            }),
        ),
        tool_call(
            3,
            "build_document",
            json!({
                "output": docx,
                "spec": {"schemaVersion": 1, "family": "docx", "blocks": [{"type": "paragraph", "text": "Typed MCP"}]},
            }),
        ),
    ];
    let responses = mcp(&requests, &[]);
    for (response, (family, output)) in responses.iter().zip([
        ("pptx", pptx.as_path()),
        ("xlsx", xlsx.as_path()),
        ("docx", docx.as_path()),
    ]) {
        assert!(response.get("error").is_none(), "{response:#}");
        let result = &response["result"]["structuredContent"];
        if family == "pptx" {
            assert_eq!(result["schemaVersion"], "ooxml-cli.pptx-build.v1");
            assert_eq!(result["output"], output.to_string_lossy().as_ref());
            assert_eq!(result["outline"]["type"], family);
            assert_eq!(result["validated"], true);
        } else {
            assert_eq!(result["family"], family);
            assert_eq!(
                result["commit"]["output"],
                output.to_string_lossy().as_ref()
            );
            assert_eq!(result["outline"]["type"], family);
            assert_eq!(result["commit"]["validated"], true);
        }
        assert!(output.is_file(), "missing built {family} output");
        let validation = run_cli_json(
            &vec![
                "--json".to_string(),
                "--strict".to_string(),
                "validate".to_string(),
                output.to_string_lossy().to_string(),
            ],
            &[],
        );
        assert_eq!(validation["valid"], true, "{validation:#}");
        assert_eq!(validation["summary"]["errors"], 0);
    }
}

#[test]
fn typed_presentation_build_matches_the_family_cli_contract() {
    let dir = temp_dir("build-parity");
    let mcp_output = dir.join("typed.pptx");
    let cli_output = dir.join("cli.pptx");
    let spec_path = dir.join("deck.json");
    let spec = json!({
        "schemaVersion": 1,
        "family": "pptx",
        "slides": [{
            "layout": "Title Slide",
            "title": "Typed parity",
            "subtitle": "One schema and one builder",
        }],
    });
    std::fs::write(
        &spec_path,
        format!("{}\n", serde_json::to_string_pretty(&spec).unwrap()),
    )
    .unwrap();

    let response = mcp(
        &[tool_call(
            1,
            "build_presentation",
            json!({"output": mcp_output, "spec": spec}),
        )],
        &[],
    )
    .remove(0);
    assert!(response.get("error").is_none(), "{response:#}");
    let mut actual = response["result"]["structuredContent"].clone();
    actual.as_object_mut().unwrap().remove("next_actions");
    normalize_strings(
        &mut actual,
        &[
            (mcp_output.to_string_lossy().as_ref(), "<output>"),
            ("inline", "<spec>"),
        ],
    );

    let mut expected = run_cli_json(
        &vec![
            "--json".to_string(),
            "pptx".to_string(),
            "build".to_string(),
            "--spec".to_string(),
            spec_path.to_string_lossy().into_owned(),
            "--out".to_string(),
            cli_output.to_string_lossy().into_owned(),
        ],
        &[],
    );
    normalize_strings(
        &mut expected,
        &[
            (cli_output.to_string_lossy().as_ref(), "<output>"),
            (spec_path.to_string_lossy().as_ref(), "<spec>"),
        ],
    );
    assert_eq!(actual, expected);
    assert_eq!(
        std::fs::read(mcp_output).unwrap(),
        std::fs::read(cli_output).unwrap()
    );
}

#[test]
fn typed_read_tools_are_byte_contract_equivalent_to_cli_outputs() {
    let file = "testdata/xlsx/minimal-workbook/workbook.xlsx";
    let render_dir = temp_dir("render");
    let requests = vec![
        tool_call(
            1,
            "outline_package",
            json!({"file": file, "depth": 1, "textPreview": 24, "sheet": "Sheet1"}),
        ),
        tool_call(
            2,
            "check_package",
            json!({"file": file, "openXmlSdk": "skip", "failOn": "error"}),
        ),
        tool_call(3, "validate_package", json!({"file": file})),
        tool_call(
            4,
            "find_text",
            json!({"file": file, "query": "Hello", "type": "text"}),
        ),
        tool_call(
            5,
            "render_preview",
            json!({"file": file, "out": render_dir, "dpi": 96}),
        ),
    ];
    let responses = mcp(&requests, &[("OOXML_RUST_MOCK_RENDER", "1")]);
    let actual = responses
        .iter()
        .map(|response| {
            let mut value = response["result"]["structuredContent"].clone();
            value.as_object_mut().unwrap().remove("next_actions");
            value
        })
        .collect::<Vec<_>>();
    let expected = vec![
        run_cli_json(
            &strings(&[
                "--json",
                "outline",
                file,
                "--depth",
                "1",
                "--text-preview",
                "24",
                "--sheet",
                "Sheet1",
            ]),
            &[],
        ),
        run_cli_json(
            &strings(&[
                "--json",
                "check",
                file,
                "--openxml-sdk",
                "skip",
                "--fail-on",
                "error",
            ]),
            &[],
        ),
        run_cli_json(&strings(&["--json", "--strict", "validate", file]), &[]),
        run_cli_json(
            &strings(&["--json", "find", "Hello", file, "--type", "text"]),
            &[],
        ),
        run_cli_json(
            &vec![
                "--json".to_string(),
                "render".to_string(),
                file.to_string(),
                "--out".to_string(),
                render_dir.to_string_lossy().to_string(),
                "--dpi".to_string(),
                "96".to_string(),
            ],
            &[("OOXML_RUST_MOCK_RENDER", "1")],
        ),
    ];
    assert_eq!(actual, expected);

    let base64_render = mcp(
        &[tool_call(
            6,
            "render_preview",
            json!({"file": file, "out": temp_dir("base64"), "includeBase64": true}),
        )],
        &[("OOXML_RUST_MOCK_RENDER", "1")],
    );
    let page = &base64_render[0]["result"]["structuredContent"]["pages"][0];
    assert!(
        page["imagePath"]
            .as_str()
            .is_some_and(|path| Path::new(path).is_file())
    );
    assert!(
        page["imageBase64"]
            .as_str()
            .is_some_and(|value| value.starts_with("bW9jay1pbWFnZS12MT") && value.len() % 4 == 0)
    );
}

#[test]
fn typed_edit_replace_and_errors_are_one_call_and_teaching() {
    let dir = temp_dir("edit");
    let input = "testdata/xlsx/minimal-workbook/workbook.xlsx";
    let edited = dir.join("edited.xlsx");
    let replaced = dir.join("replaced.xlsx");
    let responses = mcp(
        &[
            tool_call(
                1,
                "edit_package",
                json!({
                    "file": input,
                    "output": edited,
                    "operations": [{
                        "id": "changed_cell",
                        "command": "xlsx cells set",
                        "args": {"sheet": "1", "cell": "A1", "value": "Edited by typed MCP"},
                    }],
                }),
            ),
            tool_call(
                2,
                "replace_text",
                json!({
                    "file": input,
                    "query": "Hello",
                    "replacement": "Replaced by typed MCP",
                    "output": replaced,
                }),
            ),
            tool_call(
                3,
                "outline_package",
                json!({"file": input, "textPrevew": 10}),
            ),
            tool_call(
                4,
                "build_presentation",
                json!({
                    "output": dir.join("invalid-spec.pptx"),
                    "spec": {"schemaVersion": 1, "family": "pptx", "slides": [
                        {"layout": "Title Slide", "titel": "Misspelled title"}
                    ]},
                }),
            ),
        ],
        &[],
    );
    let edit = &responses[0]["result"]["structuredContent"];
    assert_eq!(edit["commit"]["opsCount"], 1);
    assert_eq!(edit["commit"]["validated"], true);
    assert_eq!(edit["commit"]["applied"][0]["id"], "changed_cell");
    assert!(edited.is_file());
    assert!(replaced.is_file());
    assert_eq!(xlsx_a1(&edited), "Edited by typed MCP");
    assert_eq!(xlsx_a1(&replaced), "Replaced by typed MCP");

    let typo = &responses[2]["result"];
    assert_eq!(typo["isError"], true);
    assert_eq!(typo["structuredContent"]["error"]["code"], "invalid_args");
    assert_eq!(
        typo["structuredContent"]["error"]["didYouMean"],
        json!(["textPreview"])
    );
    assert!(
        typo["structuredContent"]["error"]["validFields"]
            .as_array()
            .unwrap()
            .contains(&json!("file"))
    );

    let invalid_spec = &responses[3]["result"];
    assert_eq!(invalid_spec["isError"], true);
    assert_eq!(
        invalid_spec["structuredContent"]["error"]["diagnostics"][0]["code"],
        "BUILD_SPEC_UNKNOWN_FIELD"
    );
    assert_eq!(
        invalid_spec["structuredContent"]["error"]["diagnostics"][0]["didYouMean"],
        json!(["title"])
    );
    assert_eq!(
        invalid_spec["structuredContent"]["error"]["schemaResource"],
        "resource://schema/pptx-build"
    );
}

fn xlsx_a1(file: &Path) -> String {
    let value = run_cli_json(
        &vec![
            "--json".to_string(),
            "xlsx".to_string(),
            "cells".to_string(),
            "extract".to_string(),
            file.to_string_lossy().to_string(),
            "--sheet".to_string(),
            "1".to_string(),
            "--range".to_string(),
            "A1".to_string(),
        ],
        &[],
    );
    value["sheet"]["rows"][0]["cells"][0]["value"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

fn rpc(id: u32, method: &str, params: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})
}

fn tool_call(id: u32, name: &str, arguments: Value) -> Value {
    rpc(
        id,
        "tools/call",
        json!({"name": name, "arguments": arguments}),
    )
}

fn mcp(requests: &[Value], env: &[(&str, &str)]) -> Vec<Value> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ooxml"));
    command
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped());
    for (name, value) in env {
        command.env(name, value);
    }
    let mut child = command.spawn().expect("spawn MCP server");
    {
        let stdin = child.stdin.as_mut().expect("MCP stdin");
        for request in requests {
            writeln!(stdin, "{}", serde_json::to_string(request).unwrap()).unwrap();
        }
    }
    drop(child.stdin.take());
    let output = child.wait_with_output().expect("wait for MCP server");
    assert!(
        output.status.success(),
        "MCP stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(responses.len(), requests.len());
    responses
}

fn run_cli_json(args: &[String], env: &[(&str, &str)]) -> Value {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ooxml"));
    command.args(args);
    for (name, value) in env {
        command.env(name, value);
    }
    let output = command.output().expect("run ooxml CLI");
    assert!(
        output.status.success(),
        "ooxml {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("CLI JSON output")
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

fn normalize_strings(value: &mut Value, replacements: &[(&str, &str)]) {
    match value {
        Value::String(text) => {
            for (from, to) in replacements {
                *text = text.replace(from, to);
            }
        }
        Value::Array(values) => {
            for value in values {
                normalize_strings(value, replacements);
            }
        }
        Value::Object(fields) => {
            for value in fields.values_mut() {
                normalize_strings(value, replacements);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn temp_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "ooxml-typed-mcp-{label}-{}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn typed_tool_names_are_unique() {
    assert_eq!(
        TYPED_NAMES.iter().copied().collect::<BTreeSet<_>>().len(),
        TYPED_NAMES.len()
    );
}
