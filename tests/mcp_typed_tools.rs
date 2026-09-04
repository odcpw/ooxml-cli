use ooxml_cli::build::{BuildFamily, load_spec_str};
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
const TYPED_CLI_EQUIVALENTS: [(&str, &str); 10] = [
    ("build_presentation", "ooxml pptx build"),
    ("build_workbook", "ooxml xlsx build"),
    ("build_document", "ooxml docx build"),
    ("edit_package", "ooxml apply"),
    ("outline_package", "ooxml outline"),
    ("check_package", "ooxml check"),
    ("validate_package", "ooxml validate"),
    ("render_preview", "ooxml render"),
    ("find_text", "ooxml find"),
    ("replace_text", "ooxml find"),
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
    assert_eq!(capabilities["mcp"]["typedTools"], json!(TYPED_NAMES));
    assert_eq!(
        capabilities["mcp"]["genericTools"],
        json!([
            "open", "op", "inspect", "validate", "plan", "commit", "abort"
        ])
    );
    assert!(
        capabilities["mcp"]["resources"]
            .as_array()
            .unwrap()
            .contains(&json!("resource://schema/pptx-build"))
    );
    let mcp_command = capabilities["commands"]
        .as_array()
        .unwrap()
        .iter()
        .find(|command| command["path"] == "ooxml mcp")
        .expect("MCP command manifest row");
    assert!(
        mcp_command["short"]
            .as_str()
            .unwrap_or_default()
            .contains("typed build, edit, outline, check")
    );
    let guide = run_cli_json(&strings(&["--json", "robot-docs", "guide"]), &[]);
    assert!(guide["sections"].as_array().unwrap().iter().any(|section| {
        section["name"] == "MCP one-call typed intents"
            && section["commands"]
                .as_array()
                .unwrap()
                .iter()
                .any(|command| command == "MCP edit_package -> ordered manifest-derived mutation batch with named $ref results")
    }));
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
        assert_eq!(
            tool["inputSchema"]["allOf"][0]["oneOf"][0]["required"],
            json!(["spec"])
        );
        assert_eq!(
            tool["inputSchema"]["allOf"][0]["oneOf"][1]["required"],
            json!(["markdown"])
        );
        assert!(
            tool["inputSchema"]["properties"]["markdown"]["description"]
                .as_str()
                .is_some_and(|description| description.contains("Inline Markdown source"))
        );
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
fn typed_build_input_schemas_accept_markdown_and_spec_exclusively() {
    let cases = [
        (
            "build_presentation",
            BuildFamily::Pptx,
            json!({
                "schemaVersion": 1,
                "family": "pptx",
                "slides": [{"layout": "Title Slide", "title": "Schema"}],
            }),
        ),
        (
            "build_workbook",
            BuildFamily::Xlsx,
            json!({
                "schemaVersion": 1,
                "family": "xlsx",
                "sheets": [{"name": "Data"}],
            }),
        ),
        (
            "build_document",
            BuildFamily::Docx,
            json!({
                "schemaVersion": 1,
                "family": "docx",
                "blocks": [{"type": "paragraph", "text": "Schema"}],
            }),
        ),
    ];
    let tools = mcp(&[rpc(1, "tools/list", json!({}))], &[]).remove(0);
    let tools = tools["result"]["tools"]
        .as_array()
        .expect("MCP tools array");
    let requests = cases
        .iter()
        .flat_map(|(name, family, spec)| {
            let tool = tools
                .iter()
                .find(|tool| tool["name"] == *name)
                .unwrap_or_else(|| panic!("missing typed build tool {name}"));
            let schema = &tool["inputSchema"];
            let spec_text = serde_json::to_string(spec).expect("serialize build spec");
            assert!(
                load_spec_str(*family, &spec_text).is_ok(),
                "test spec must match the published {} schema",
                family.schema_name()
            );

            let output = "schema-output";
            let markdown = "# Schema input\n";
            let accepted = [
                (
                    "spec + output",
                    json!({"spec": spec, "output": output}),
                    true,
                ),
                (
                    "markdown + output",
                    json!({"markdown": markdown, "output": output}),
                    true,
                ),
                (
                    "spec + session",
                    json!({"spec": spec, "session": "schema-session"}),
                    true,
                ),
                (
                    "markdown + session",
                    json!({"markdown": markdown, "session": "schema-session"}),
                    true,
                ),
                (
                    "both sources",
                    json!({
                        "spec": spec,
                        "markdown": markdown,
                        "output": output,
                    }),
                    false,
                ),
                ("neither source", json!({"output": output}), false),
                (
                    "both destinations",
                    json!({"spec": spec, "output": output, "session": "schema-session"}),
                    false,
                ),
                ("neither destination", json!({"spec": spec}), false),
            ];
            for (label, request, expected) in accepted {
                assert_eq!(
                    build_input_schema_accepts(schema, &request),
                    expected,
                    "{name} schema acceptance drifted for {label}: {request}"
                );
            }

            [
                json!({
                    "spec": spec,
                    "markdown": markdown,
                    "output": output,
                }),
                json!({"output": output}),
            ]
            .into_iter()
            .map(|arguments| tool_call(1, name, arguments))
            .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let responses = mcp(&requests, &[]);
    for response in responses {
        let result = &response["result"];
        assert_eq!(result["isError"], true, "invalid build request succeeded");
        assert_eq!(
            result["structuredContent"]["error"]["message"],
            "exactly one of spec or markdown is required"
        );
    }
}

fn build_input_schema_accepts(schema: &Value, request: &Value) -> bool {
    let Some(request) = request.as_object() else {
        return false;
    };
    if schema["type"] != "object" || schema["additionalProperties"] != false {
        return false;
    }
    let Some(properties) = schema["properties"].as_object() else {
        return false;
    };
    if request.keys().any(|field| !properties.contains_key(field)) {
        return false;
    }
    schema["allOf"].as_array().is_some_and(|clauses| {
        clauses.iter().all(|clause| {
            let Some(variants) = clause["oneOf"].as_array() else {
                return false;
            };
            variants
                .iter()
                .filter(|variant| schema_required_and_not_match(variant, request))
                .count()
                == 1
        })
    })
}

fn schema_required_and_not_match(schema: &Value, request: &serde_json::Map<String, Value>) -> bool {
    let required = schema["required"].as_array().is_none_or(|fields| {
        fields
            .iter()
            .filter_map(Value::as_str)
            .all(|field| request.contains_key(field))
    });
    let not_match = schema["not"].as_object().is_none_or(|not| {
        not["required"].as_array().is_none_or(|fields| {
            !fields
                .iter()
                .filter_map(Value::as_str)
                .all(|field| request.contains_key(field))
        })
    });
    required && not_match
}

#[test]
fn resources_list_publishes_build_schemas_and_the_real_typed_recipe_guide() {
    let responses = mcp(
        &[
            rpc(1, "resources/list", json!({})),
            rpc(
                2,
                "resources/read",
                json!({"uri": "resource://agent-guide"}),
            ),
        ],
        &[],
    );
    let resources = responses[0]["result"]["resources"].as_array().unwrap();
    for uri in [
        "resource://agent-guide",
        "resource://schema/pptx-build",
        "resource://schema/xlsx-build",
        "resource://schema/docx-build",
    ] {
        assert!(
            resources.iter().any(|resource| resource["uri"] == uri),
            "missing MCP resource {uri}"
        );
    }
    let guide_text = responses[1]["result"]["contents"][0]["text"]
        .as_str()
        .expect("agent guide resource text");
    let guide: Value = serde_json::from_str(guide_text).expect("agent guide JSON");
    let typed = guide["sections"]
        .as_array()
        .unwrap()
        .iter()
        .find(|section| section["name"] == "MCP one-call typed intents")
        .expect("typed MCP recipe section");
    assert!(typed["commands"].as_array().unwrap().iter().any(|command| {
        command.as_str().is_some_and(|command| {
            command.contains("build_presentation|build_workbook|build_document")
        })
    }));
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
        assert_eq!(
            result["schemaVersion"],
            format!("ooxml-cli.{family}-build.v1")
        );
        assert_eq!(result["output"], output.to_string_lossy().as_ref());
        assert_eq!(result["outline"]["type"], family);
        assert_eq!(result["validated"], true);
        assert!(output.is_file(), "missing built {family} output");
        let validation = run_cli_json(
            &[
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
fn typed_build_tools_match_the_family_cli_contracts() {
    let dir = temp_dir("build-parity");
    let cases = [
        (
            "pptx",
            "build_presentation",
            "pptx",
            json!({
                "schemaVersion": 1,
                "family": "pptx",
                "slides": [{
                    "layout": "Title Slide",
                    "title": "Typed parity",
                    "subtitle": "One schema and one builder",
                }],
            }),
        ),
        (
            "xlsx",
            "build_workbook",
            "xlsx",
            json!({
                "schemaVersion": 1,
                "family": "xlsx",
                "sheets": [{"name": "Data"}],
            }),
        ),
        (
            "docx",
            "build_document",
            "docx",
            json!({
                "schemaVersion": 1,
                "family": "docx",
                "blocks": [{"type": "paragraph", "text": "Typed parity"}],
            }),
        ),
    ];
    for (family, tool, extension, spec) in cases {
        let mcp_output = dir.join(format!("typed.{extension}"));
        let cli_output = dir.join(format!("cli.{extension}"));
        let spec_path = dir.join(format!("{family}.json"));
        std::fs::write(
            &spec_path,
            format!("{}\n", serde_json::to_string_pretty(&spec).unwrap()),
        )
        .unwrap();

        let response = mcp(
            &[tool_call(
                1,
                tool,
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
            &[
                "--json".to_string(),
                family.to_string(),
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
                (&format!("<spec-dir>/cli.{extension}"), "<output>"),
                (&format!("<spec-dir>/{family}.json"), "<spec>"),
            ],
        );
        assert_eq!(actual, expected, "{family} MCP/CLI build parity drifted");
        assert_eq!(
            std::fs::read(mcp_output).unwrap(),
            std::fs::read(cli_output).unwrap(),
            "{family} MCP/CLI artifacts differ"
        );
    }
}

#[test]
fn typed_markdown_builds_match_family_cli_and_xlsx_refuses_with_a_teaching_error() {
    let dir = temp_dir("markdown-build-parity");
    let cases = [
        (
            "pptx",
            "build_presentation",
            "pptx",
            "# Typed review\n\n---\n\n## Delivery\n\nThe release is on track.\n",
        ),
        (
            "docx",
            "build_document",
            "docx",
            "# Typed report\n\nThe release is on track.\n\n- Proof is recorded\n- Follow-up is explicit\n",
        ),
    ];
    for (family, tool, extension, markdown) in cases {
        let mcp_output = dir.join(format!("typed-markdown.{extension}"));
        let cli_output = dir.join(format!("cli-markdown.{extension}"));
        let markdown_path = dir.join(format!("{family}.md"));
        std::fs::write(&markdown_path, markdown).unwrap();

        let response = mcp(
            &[tool_call(
                1,
                tool,
                json!({"output": mcp_output, "markdown": markdown}),
            )],
            &[],
        )
        .remove(0);
        assert!(response.get("error").is_none(), "{response:#}");
        assert_ne!(response["result"]["isError"], true, "{response:#}");
        let mut actual = response["result"]["structuredContent"].clone();
        actual.as_object_mut().unwrap().remove("next_actions");
        normalize_strings(
            &mut actual,
            &[(mcp_output.to_string_lossy().as_ref(), "<output>")],
        );

        let mut expected = run_cli_json(
            &[
                "--json".to_string(),
                family.to_string(),
                "build".to_string(),
                "--from-markdown".to_string(),
                markdown_path.to_string_lossy().into_owned(),
                "--out".to_string(),
                cli_output.to_string_lossy().into_owned(),
            ],
            &[],
        );
        normalize_strings(
            &mut expected,
            &[
                (cli_output.to_string_lossy().as_ref(), "<output>"),
                (markdown_path.to_string_lossy().as_ref(), "inline"),
                (&format!("<spec-dir>/cli-markdown.{extension}"), "<output>"),
            ],
        );
        assert_eq!(actual, expected, "{family} Markdown MCP/CLI parity drifted");
        assert_eq!(
            std::fs::read(&mcp_output).unwrap(),
            std::fs::read(&cli_output).unwrap(),
            "{family} Markdown artifacts differ"
        );
        let validation = run_cli_json(
            &[
                "--json".to_string(),
                "--strict".to_string(),
                "validate".to_string(),
                mcp_output.to_string_lossy().into_owned(),
            ],
            &[],
        );
        assert_eq!(validation["valid"], true, "{validation:#}");
    }

    let unsupported = mcp(
        &[tool_call(
            2,
            "build_workbook",
            json!({
                "output": dir.join("unsupported.xlsx"),
                "markdown": "# Tabular data\n",
            }),
        )],
        &[],
    )
    .remove(0);
    let error = &unsupported["result"]["structuredContent"]["error"];
    assert_eq!(unsupported["result"]["isError"], true);
    assert_eq!(error["code"], "invalid_args");
    assert_eq!(error["diagnostics"]["code"], "MARKDOWN_FAMILY_UNSUPPORTED");
    assert!(
        error["message"]
            .as_str()
            .is_some_and(|message| message.contains("not xlsx")),
        "{error:#}"
    );
    assert_eq!(error["schemaResource"], "resource://schema/xlsx-build");
}

#[test]
fn typed_build_tools_reject_ambiguous_or_inapplicable_fields_instead_of_dropping_them() {
    let dir = temp_dir("build-fields");
    let workbook_spec = json!({
        "schemaVersion": 1,
        "family": "xlsx",
        "sheets": [{"name": "Data"}],
    });
    let document_spec = json!({
        "schemaVersion": 1,
        "family": "docx",
        "blocks": [{"type": "paragraph", "text": "No silent drops"}],
    });
    let responses = mcp(
        &[
            tool_call(
                1,
                "build_workbook",
                json!({
                    "spec": workbook_spec.clone(),
                    "output": dir.join("ambiguous.xlsx"),
                    "session": "existing-session",
                }),
            ),
            tool_call(
                2,
                "build_document",
                json!({
                    "spec": document_spec,
                    "session": "existing-session",
                    "check": true,
                }),
            ),
            tool_call(
                3,
                "build_workbook",
                json!({
                    "spec": workbook_spec,
                    "output": dir.join("typo.xlsx"),
                    "chek": true,
                }),
            ),
            tool_call(
                4,
                "build_document",
                json!({
                    "spec": {
                        "schemaVersion": 1,
                        "family": "docx",
                        "blocks": [{"type": "paragraph", "text": "ambiguous"}],
                    },
                    "markdown": "# Ambiguous\n",
                    "output": dir.join("ambiguous.docx"),
                }),
            ),
        ],
        &[],
    );

    let ambiguous = &responses[0]["result"];
    assert_eq!(ambiguous["isError"], true);
    assert_eq!(
        ambiguous["structuredContent"]["error"]["message"],
        "exactly one of output or session is required"
    );
    let session_flag = &responses[1]["result"];
    assert_eq!(session_flag["isError"], true);
    assert!(
        session_flag["structuredContent"]["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("check, dryRun, and force apply only"))
    );
    let typo = &responses[2]["result"];
    assert_eq!(typo["isError"], true);
    assert_eq!(
        typo["structuredContent"]["error"]["didYouMean"],
        json!(["check"])
    );
    assert!(
        typo["structuredContent"]["error"]["validFields"]
            .as_array()
            .unwrap()
            .contains(&json!("check"))
    );
    let ambiguous_source = &responses[3]["result"];
    assert_eq!(ambiguous_source["isError"], true);
    assert_eq!(
        ambiguous_source["structuredContent"]["error"]["message"],
        "exactly one of spec or markdown is required"
    );
}

#[test]
fn typed_build_can_apply_a_full_family_plan_to_a_session_and_commit_it() {
    let dir = temp_dir("build-session");
    let source = dir.join("new-presentation.pptx");
    let output = dir.join("session-built.pptx");
    let responses = mcp(
        &[
            tool_call(1, "open", json!({"file": source, "out": output})),
            tool_call(
                2,
                "build_presentation",
                json!({
                    "session": "rust-session-1",
                    "spec": {
                        "schemaVersion": 1,
                        "family": "pptx",
                        "slides": [
                            {"layout": "Title Slide", "title": "First", "subtitle": "Built in session"},
                            {"layout": "Title Slide", "title": "Second", "subtitle": "The family compiler keeps this second slide."},
                        ],
                    },
                }),
            ),
            tool_call(3, "commit", json!({"session": "rust-session-1"})),
        ],
        &[],
    );
    for response in &responses {
        assert!(response.get("error").is_none(), "{response:#}");
        assert_ne!(response["result"]["isError"], true, "{response:#}");
    }
    let build = &responses[1]["result"]["structuredContent"];
    assert_eq!(build["family"], "pptx");
    assert_eq!(build["committed"], false);
    assert!(
        build["operations"]
            .as_array()
            .is_some_and(|operations| operations.len() >= 3),
        "full two-slide plan was not applied: {build:#}"
    );
    let commit = &responses[2]["result"]["structuredContent"];
    assert_eq!(commit["validated"], true);
    assert_eq!(commit["output"], output.to_string_lossy().as_ref());
    let outline = run_cli_json(
        &[
            "--json".to_string(),
            "outline".to_string(),
            output.to_string_lossy().into_owned(),
        ],
        &[],
    );
    assert_eq!(outline["summary"]["slides"], 2);
}

#[test]
fn typed_markdown_can_apply_a_compiled_plan_to_a_session() {
    let dir = temp_dir("markdown-session");
    let source = dir.join("new-document.docx");
    let output = dir.join("session-markdown.docx");
    let responses = mcp(
        &[
            tool_call(1, "open", json!({"file": source, "out": output})),
            tool_call(
                2,
                "build_document",
                json!({
                    "session": "rust-session-1",
                    "markdown": "# Session report\n\nBuilt from Markdown without an intermediate spec file.\n",
                }),
            ),
            tool_call(3, "commit", json!({"session": "rust-session-1"})),
        ],
        &[],
    );
    for response in &responses {
        assert!(response.get("error").is_none(), "{response:#}");
        assert_ne!(response["result"]["isError"], true, "{response:#}");
    }
    let build = &responses[1]["result"]["structuredContent"];
    assert_eq!(build["family"], "docx");
    assert_eq!(build["markdown"], "inline");
    assert_eq!(build["committed"], false);
    assert!(
        build["operations"]
            .as_array()
            .is_some_and(|operations| operations.len() >= 4),
        "full Markdown plan was not applied: {build:#}"
    );
    assert_eq!(
        responses[2]["result"]["structuredContent"]["validated"],
        true
    );
    let outline = run_cli_json(
        &[
            "--json".to_string(),
            "outline".to_string(),
            output.to_string_lossy().into_owned(),
        ],
        &[],
    );
    assert_eq!(outline["summary"]["paragraphs"], 2);
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
            &[
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
fn typed_cli_parity_matrix_covers_every_published_tool_and_command() {
    let mapped_tools = TYPED_CLI_EQUIVALENTS
        .iter()
        .map(|(tool, _)| *tool)
        .collect::<BTreeSet<_>>();
    assert_eq!(mapped_tools, TYPED_NAMES.into_iter().collect());

    let capabilities = run_cli_json(&strings(&["--json", "capabilities"]), &[]);
    let command_paths = capabilities["commands"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|command| command["path"].as_str())
        .collect::<BTreeSet<_>>();
    for (tool, command) in TYPED_CLI_EQUIVALENTS {
        assert!(
            command_paths.contains(command),
            "typed tool {tool} maps to missing CLI command {command}"
        );
    }
}

#[test]
fn json_rpc_errors_and_typed_tool_failures_keep_protocol_boundaries() {
    let responses = mcp(
        &[
            rpc(1, "unsupported/method", json!({})),
            tool_call(2, "not_a_tool", json!({})),
            tool_call(3, "outline_package", json!({})),
        ],
        &[],
    );
    assert_eq!(responses[0]["error"]["code"], -32601);
    assert_eq!(responses[0]["error"]["data"]["exitCode"], 2);
    assert_eq!(responses[1]["error"]["code"], -32602);
    assert_eq!(responses[1]["error"]["data"]["exitCode"], 2);

    assert!(responses[2].get("error").is_none());
    assert_eq!(responses[2]["result"]["isError"], true);
    assert_eq!(
        responses[2]["result"]["structuredContent"]["error"]["code"],
        "invalid_args"
    );
    assert_eq!(
        responses[2]["result"]["structuredContent"]["error"]["exitCode"],
        2
    );
}

#[test]
fn oversized_typed_output_spills_to_configured_lf_json_file() {
    let output_dir = temp_dir("output-spill");
    let response = mcp(
        &[tool_call(
            41,
            "outline_package",
            json!({
                "file": "testdata/pptx/edge-large-deck/presentation.pptx",
                "depth": 2,
                "textPreview": 80,
            }),
        )],
        &[
            ("OOXML_MCP_MAX_OUTPUT_BYTES", "512"),
            (
                "OOXML_MCP_OUTPUT_DIR",
                output_dir.to_string_lossy().as_ref(),
            ),
        ],
    )
    .remove(0);
    let pointer = &response["result"]["structuredContent"];
    assert_eq!(pointer["truncated"], true);
    assert_eq!(pointer["mimeType"], "application/json");
    let output_file = PathBuf::from(pointer["outputFile"].as_str().unwrap());
    assert_eq!(output_file, output_dir.join("mcp-response-000001.json"));
    let bytes = std::fs::read(&output_file).expect("read externalized MCP response");
    assert_eq!(pointer["byteCount"], bytes.len());
    assert!(bytes.ends_with(b"\n"));
    assert!(!bytes.contains(&b'\r'));
    let full: Value = serde_json::from_slice(&bytes).expect("externalized JSON-RPC response");
    assert_eq!(full["id"], 41);
    assert_ne!(full["result"]["isError"], true);
    assert_eq!(full["result"]["structuredContent"]["type"], "pptx");
    assert!(
        full["result"]["structuredContent"]["slides"]
            .as_array()
            .is_some_and(|slides| slides.len() > 10)
    );
}

#[test]
fn flue_beta9_and_smokes_require_the_typed_check_tool_where_applicable() {
    let package: Value =
        serde_json::from_str(&std::fs::read_to_string("web/package.json").unwrap()).unwrap();
    for path in [
        "/dependencies/@flue~1runtime",
        "/dependencies/@flue~1sdk",
        "/devDependencies/@flue~1cli",
    ] {
        assert_eq!(package.pointer(path).unwrap(), "1.0.0-beta.9");
    }
    assert_eq!(package["overrides"]["undici"], "7.29.0");

    let tools = std::fs::read_to_string("web/src/shared/ooxml-tools.ts").unwrap();
    assert!(tools.contains("input: v.object({"));
    assert!(tools.contains("run: async ({ input:"));
    assert!(!tools.contains("parameters:"));
    assert!(!tools.contains("execute:"));

    let tool_smoke = std::fs::read_to_string("web/scripts/smoke-flue-tools.mjs").unwrap();
    assert!(tool_smoke.contains("get_ooxml_capabilities"));
    assert!(tool_smoke.contains("check_package"));
    assert!(tool_smoke.contains("tool.run({ input })"));

    let non_pptx = std::fs::read_to_string("web/scripts/smoke-nonpptx.mjs").unwrap();
    let agent_edit = std::fs::read_to_string("web/scripts/smoke-agent-edit.mjs").unwrap();
    assert!(non_pptx.contains("check_package"));
    assert!(non_pptx.contains("tools/call"));
    for required in [
        "get_ooxml_capabilities",
        "inspect_current_with_ooxml",
        "apply_ooxml_ops_to_current",
        "check_package",
    ] {
        assert!(agent_edit.contains(required));
    }
    assert!(agent_edit.contains("smoke:agent requires OPENAI_API_KEY"));
    assert!(!agent_edit.contains("summary.toolNames.length &&"));
}

#[test]
fn typed_edit_replace_and_errors_are_one_call_and_teaching() {
    let dir = temp_dir("edit");
    let input = "testdata/xlsx/minimal-workbook/workbook.xlsx";
    let edited = dir.join("edited.xlsx");
    let replaced = dir.join("replaced.xlsx");
    let direct_edit = dir.join("direct-edited.xlsx");
    let direct_replace = dir.join("direct-replaced.xlsx");
    let edit_operations = json!([{
        "id": "changed_cell",
        "command": "xlsx cells set",
        "args": {"sheet": "1", "cell": "A1", "value": "Edited by typed MCP"},
    }]);
    let responses = mcp(
        &[
            tool_call(
                1,
                "edit_package",
                json!({
                    "file": input,
                    "output": edited,
                    "operations": edit_operations,
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

    let ops_path = dir.join("ops.json");
    std::fs::write(
        &ops_path,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&edit_operations).unwrap()
        ),
    )
    .unwrap();
    let mut direct_edit_result = run_cli_json(
        &[
            "--json".to_string(),
            "apply".to_string(),
            input.to_string(),
            "--ops".to_string(),
            ops_path.to_string_lossy().into_owned(),
            "--out".to_string(),
            direct_edit.to_string_lossy().into_owned(),
        ],
        &[],
    );
    let direct_outer_envelope = direct_edit_result
        .as_object_mut()
        .unwrap()
        .remove("mutationEnvelope")
        .expect("CLI apply outer mutation envelope");
    assert_eq!(direct_outer_envelope["validated"], true);
    assert_eq!(direct_outer_envelope["destination"]["kind"], "batch");
    assert_eq!(
        direct_outer_envelope["destination"]["primarySelector"],
        edit["commit"]["applied"][0]["mutationEnvelope"]["destination"]["primarySelector"]
    );
    let mut typed_edit_commit = edit["commit"].clone();
    normalize_strings(
        &mut typed_edit_commit,
        &[(edited.to_string_lossy().as_ref(), "<output>")],
    );
    normalize_strings(
        &mut direct_edit_result,
        &[(direct_edit.to_string_lossy().as_ref(), "<output>")],
    );
    assert_eq!(typed_edit_commit, direct_edit_result);
    assert_eq!(
        std::fs::read(&edited).unwrap(),
        std::fs::read(&direct_edit).unwrap()
    );

    let mut direct_replace_result = run_cli_json(
        &[
            "--json".to_string(),
            "find".to_string(),
            "Hello".to_string(),
            input.to_string(),
            "--replace".to_string(),
            "Replaced by typed MCP".to_string(),
            "--apply".to_string(),
            "--out".to_string(),
            direct_replace.to_string_lossy().into_owned(),
        ],
        &[],
    );
    let mut typed_replace_result = responses[1]["result"]["structuredContent"].clone();
    typed_replace_result
        .as_object_mut()
        .unwrap()
        .remove("next_actions");
    let direct_replace_outer = direct_replace_result
        .as_object_mut()
        .unwrap()
        .remove("mutationEnvelope")
        .expect("CLI find --apply outer mutation envelope");
    assert_eq!(direct_replace_outer["validated"], true);
    assert_eq!(direct_replace_outer["destination"]["kind"], "text-match");
    assert_eq!(
        direct_replace_outer["destination"]["primarySelector"],
        typed_replace_result["applied"][0]["mutationEnvelope"]["destination"]["primarySelector"]
    );
    normalize_strings(
        &mut typed_replace_result,
        &[(replaced.to_string_lossy().as_ref(), "<output>")],
    );
    normalize_strings(
        &mut direct_replace_result,
        &[(direct_replace.to_string_lossy().as_ref(), "<output>")],
    );
    assert_eq!(typed_replace_result, direct_replace_result);
    assert_eq!(
        std::fs::read(&replaced).unwrap(),
        std::fs::read(&direct_replace).unwrap()
    );

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
        &[
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
