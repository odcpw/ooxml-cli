use ooxml_cli::build::{
    BuildCompiler, BuildFamily, ImageRef, compile_minimal_spec, load_spec_str, operation_reference,
    schema_by_name,
};
use serde_json::{Map, Value, json};
use std::collections::BTreeSet;
use std::io::Write;
use std::process::{Command, Stdio};

const SCHEMA_INDEX: &str = include_str!("../testdata/golden/build-spec/schema-index.json");

#[test]
fn published_family_schemas_match_the_pinned_index() {
    let expected: Value = serde_json::from_str(SCHEMA_INDEX).expect("schema index golden");
    let actual = Value::Array(
        BuildFamily::ALL
            .into_iter()
            .map(|family| {
                let schema = schema_by_name(family.schema_name()).expect("published schema");
                assert_eq!(
                    schema["$schema"],
                    "https://json-schema.org/draft/2020-12/schema"
                );
                assert_eq!(schema["type"], "object");
                assert_eq!(schema["additionalProperties"], false);
                let family_definitions = match family {
                    BuildFamily::Pptx => BTreeSet::from(["pptxSlide"]),
                    BuildFamily::Xlsx => BTreeSet::from(["typedColumn", "xlsxSheet"]),
                    BuildFamily::Docx => BTreeSet::from(["docxBlock", "docxSection"]),
                };
                let definitions = schema["$defs"].as_object().expect("schema definitions");
                let common = definitions
                    .keys()
                    .filter(|name| !family_definitions.contains(name.as_str()))
                    .cloned()
                    .collect::<Vec<_>>();
                json!({
                    "name": family.schema_name(),
                    "id": schema["$id"],
                    "required": schema["required"],
                    "familyDefinitions": family_definitions,
                    "commonDefinitions": common,
                })
            })
            .collect::<Vec<_>>(),
    );
    assert_eq!(actual, expected, "published build schema index drifted");
}

#[test]
fn capabilities_publishes_each_pinned_build_schema() {
    for family in BuildFamily::ALL {
        let output = Command::new(env!("CARGO_BIN_EXE_ooxml"))
            .args(["--json", "capabilities", "--schema", family.schema_name()])
            .output()
            .expect("run capabilities schema command");
        assert!(
            output.status.success(),
            "{} schema stderr: {}",
            family,
            String::from_utf8_lossy(&output.stderr)
        );
        let actual: Value = serde_json::from_slice(&output.stdout).expect("schema JSON stdout");
        assert_eq!(actual, schema_by_name(family.schema_name()).unwrap());
    }

    let output = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args(["--json", "capabilities"])
        .output()
        .expect("run capabilities inventory");
    assert!(output.status.success());
    let capabilities: Value = serde_json::from_slice(&output.stdout).expect("capabilities JSON");
    let command = capabilities["commands"]
        .as_array()
        .unwrap()
        .iter()
        .find(|command| command["path"] == "ooxml capabilities")
        .expect("capabilities command row");
    let schema_flag = command["localFlags"]
        .as_array()
        .unwrap()
        .iter()
        .find(|flag| flag["name"] == "--schema")
        .expect("capabilities schema flag");
    for name in ["brand", "pptx-build", "xlsx-build", "docx-build"] {
        assert!(
            schema_flag["description"].as_str().unwrap().contains(name),
            "schema flag must document {name}"
        );
    }
}

#[test]
fn mcp_lists_and_reads_each_pinned_build_schema() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn MCP server");
    let mut stdin = child.stdin.take().expect("MCP stdin");
    writeln!(
        stdin,
        "{}",
        json!({"jsonrpc":"2.0","id":1,"method":"resources/list","params":{}})
    )
    .expect("write resources/list request");
    for (index, family) in BuildFamily::ALL.into_iter().enumerate() {
        writeln!(
            stdin,
            "{}",
            json!({
                "jsonrpc": "2.0",
                "id": index + 2,
                "method": "resources/read",
                "params": {"uri": format!("resource://schema/{}", family.schema_name())}
            })
        )
        .expect("write schema read request");
    }
    drop(stdin);
    let output = child.wait_with_output().expect("wait for MCP server");
    assert!(
        output.status.success(),
        "MCP stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses = String::from_utf8(output.stdout)
        .expect("MCP UTF-8")
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("MCP JSON response"))
        .collect::<Vec<_>>();
    assert_eq!(responses.len(), 4);
    let resources = responses[0]["result"]["resources"]
        .as_array()
        .expect("MCP resources array");
    for (index, family) in BuildFamily::ALL.into_iter().enumerate() {
        let uri = format!("resource://schema/{}", family.schema_name());
        assert!(
            resources.iter().any(|resource| {
                resource["uri"] == uri && resource["mimeType"] == "application/schema+json"
            }),
            "MCP must list {uri}"
        );
        let content = &responses[index + 1]["result"]["contents"][0];
        assert_eq!(content["uri"], uri);
        assert_eq!(content["mimeType"], "application/schema+json");
        let actual: Value = serde_json::from_str(content["text"].as_str().unwrap())
            .expect("MCP schema resource JSON");
        assert_eq!(actual, schema_by_name(family.schema_name()).unwrap());
    }
}

#[test]
fn loader_accepts_common_rich_types_and_human_lengths() {
    let spec = load_spec_str(
        BuildFamily::Pptx,
        &json!({
            "schemaVersion": 1,
            "family": "pptx",
            "brand": {"path": "brand.json"},
            "slides": [{
                "layout": "Title and Content",
                "title": "Metrics",
                "bullets": [{
                    "runs": [
                        {"text": "Revenue ", "bold": true, "color": "4472C4"},
                        {"text": "details", "italic": true, "link": "https://example.com"}
                    ],
                    "level": 1,
                    "bullet": true,
                    "align": "left"
                }],
                "images": [{
                    "path": "hero.png",
                    "fit": "cover",
                    "altText": "Product hero",
                    "bounds": {"x": "5%", "y": "1in", "cx": "10cm", "cy": 1800000}
                }],
                "tables": [{
                    "rows": [["Quarter", "Revenue"], ["Q3", 42]],
                    "header": true,
                    "style": "Medium2"
                }],
                "charts": [{
                    "type": "column",
                    "categories": ["Q2", "Q3"],
                    "series": [{"name": "Revenue", "values": [38, 42]}],
                    "slot": "right"
                }]
            }]
        })
        .to_string(),
    )
    .expect("rich common vocabulary must validate");
    assert_eq!(spec.family(), BuildFamily::Pptx);

    let image: ImageRef = serde_json::from_value(spec.document()["slides"][0]["images"][0].clone())
        .expect("typed image ref");
    image
        .bounds
        .as_ref()
        .expect("image bounds")
        .x
        .validate()
        .expect("percentage length");
    assert_eq!(image.alt_text.as_deref(), Some("Product hero"));
}

#[test]
fn loader_reports_precise_unknown_field_paths_and_errors_teach_suggestions() {
    let error = load_spec_str(
        BuildFamily::Pptx,
        r#"{
          "schemaVersion": 1,
          "family": "pptx",
          "slides": [{
            "layout": "Title and Content",
            "bullets": [{"text": "Revenue", "leve": 1}]
          }]
        }"#,
    )
    .expect_err("unknown nested field must be rejected");
    assert_eq!(error.diagnostics.len(), 1, "{error:?}");
    assert_eq!(
        serde_json::to_value(&error.diagnostics[0]).expect("diagnostic JSON"),
        json!({
            "code": "BUILD_SPEC_UNKNOWN_FIELD",
            "path": "/slides/0/bullets/0/leve",
            "message": "unknown field \"leve\"; did you mean \"level\"?",
            "didYouMean": ["level"],
            "validFields": ["align", "bold", "bullet", "color", "italic", "level", "numbered", "runs", "size", "style", "text"]
        })
    );
}

#[test]
fn loader_reports_required_type_and_length_errors_at_json_pointers() {
    let missing = load_spec_str(BuildFamily::Xlsx, r#"{"schemaVersion":1,"family":"xlsx"}"#)
        .expect_err("sheets are required");
    assert_eq!(missing.diagnostics[0].path, "/sheets");
    assert_eq!(
        missing.diagnostics[0].code,
        "BUILD_SPEC_REQUIRED_FIELD_MISSING"
    );

    let invalid = load_spec_str(
        BuildFamily::Pptx,
        r#"{"schemaVersion":1,"family":"pptx","slides":[{"layout":"Blank","images":[{"path":"x.png","bounds":{"x":"later","y":0,"cx":1,"cy":1}}]}]}"#,
    )
    .expect_err("invalid human length must be rejected");
    assert_eq!(invalid.diagnostics.len(), 1, "{invalid:?}");
    assert_eq!(invalid.diagnostics[0].path, "/slides/0/images/0/bounds/x");
    assert_eq!(invalid.diagnostics[0].code, "BUILD_SPEC_VALUE_INVALID");

    let wrong_family = load_spec_str(
        BuildFamily::Docx,
        r#"{"schemaVersion":1,"family":"pptx","blocks":[{"type":"paragraph","text":"x"}]}"#,
    )
    .expect_err("family constant is schema-enforced");
    assert_eq!(wrong_family.diagnostics[0].path, "/family");
}

#[test]
fn minimal_family_specs_compile_to_pinned_batch_plans() {
    let cases = [
        (
            BuildFamily::Pptx,
            json!({
                "schemaVersion": 1,
                "family": "pptx",
                "theme": "midnight",
                "slides": [{
                    "id": "cover",
                    "layout": "Title Slide",
                    "title": "Q3 review",
                    "subtitle": "Board update"
                }]
            }),
            include_str!("../testdata/golden/build-spec/pptx-minimal-plan.json"),
        ),
        (
            BuildFamily::Xlsx,
            json!({
                "schemaVersion": 1,
                "family": "xlsx",
                "themeSeed": "4472C4",
                "sheets": [{"id": "sales", "name": "Sales"}]
            }),
            include_str!("../testdata/golden/build-spec/xlsx-minimal-plan.json"),
        ),
        (
            BuildFamily::Docx,
            json!({
                "schemaVersion": 1,
                "family": "docx",
                "theme": "corporate-blue",
                "blocks": [{
                    "id": "intro",
                    "type": "paragraph",
                    "text": "Quarterly report"
                }]
            }),
            include_str!("../testdata/golden/build-spec/docx-minimal-plan.json"),
        ),
    ];
    for (family, source, golden) in cases {
        let spec = load_spec_str(family, &source.to_string()).expect("minimal spec loads");
        let plan = compile_minimal_spec(&spec).expect("minimal spec compiles");
        let actual = serde_json::to_value(&plan).expect("plan JSON");
        let expected: Value = serde_json::from_str(golden).expect("plan golden");
        assert_eq!(actual, expected, "{family} minimal plan drifted");
        assert_eq!(plan.operations_json(), actual["operations"]);
    }
}

#[test]
fn minimal_compiler_never_silently_drops_rich_family_fields() {
    let pptx = load_spec_str(
        BuildFamily::Pptx,
        &json!({
            "schemaVersion": 1,
            "family": "pptx",
            "slides": [{
                "layout": "Title Slide",
                "title": "Metrics",
                "bullets": [{"text": "Revenue"}]
            }]
        })
        .to_string(),
    )
    .expect("rich presentation spec loads");
    let error = compile_minimal_spec(&pptx).expect_err("rich fields must not be dropped");
    assert_eq!(error.code, "BUILD_SPEC_FAMILY_COMPILER_REQUIRED");
    assert_eq!(error.path, "/slides/0/bullets");
    assert_eq!(error.op_id, None);

    let docx = load_spec_str(
        BuildFamily::Docx,
        r#"{"schemaVersion":1,"family":"docx","blocks":[{"type":"image","image":{"path":"hero.png"}}]}"#,
    )
    .expect("image document spec loads");
    let error = compile_minimal_spec(&docx).expect_err("non-paragraph block needs family compiler");
    assert_eq!(error.path, "/blocks/0/image");
}

#[test]
fn compiled_minimal_plans_are_accepted_by_apply_dry_run() {
    let temp = std::env::temp_dir().join(format!(
        "ooxml-build-spec-core-apply-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).expect("create build-spec test directory");
    let cases = [
        (
            BuildFamily::Pptx,
            json!({
                "schemaVersion": 1,
                "family": "pptx",
                "slides": [{"layout": "Title Slide", "title": "Plan"}]
            }),
            "planned.pptx",
            "pptx scaffold",
        ),
        (
            BuildFamily::Xlsx,
            json!({
                "schemaVersion": 1,
                "family": "xlsx",
                "sheets": [{"name": "Plan"}]
            }),
            "planned.xlsx",
            "xlsx scaffold",
        ),
        (
            BuildFamily::Docx,
            json!({
                "schemaVersion": 1,
                "family": "docx",
                "blocks": [{"type": "paragraph", "text": "Plan"}]
            }),
            "planned.docx",
            "docx scaffold",
        ),
    ];
    for (family, source, file_name, command) in cases {
        let spec = load_spec_str(family, &source.to_string()).expect("minimal spec loads");
        let plan = compile_minimal_spec(&spec).expect("minimal spec compiles");
        let ops = temp.join(format!("{family}-ops.json"));
        std::fs::write(
            &ops,
            serde_json::to_vec_pretty(&plan.operations).expect("serialize compiled ops"),
        )
        .expect("write compiled ops");
        let output_path = temp.join(file_name);
        let output = Command::new(env!("CARGO_BIN_EXE_ooxml"))
            .args(["--json", "apply"])
            .arg(&output_path)
            .arg("--ops")
            .arg(&ops)
            .arg("--dry-run")
            .output()
            .expect("run compiled plan through apply");
        assert!(
            output.status.success(),
            "{family} apply stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: Value = serde_json::from_slice(&output.stdout).expect("apply JSON");
        assert_eq!(result["dryRun"], true);
        assert_eq!(result["committed"], false);
        assert_eq!(result["plan"][0]["command"], command);
        assert!(
            !output_path.exists(),
            "dry-run must not publish {file_name}"
        );
    }
    let _ = std::fs::remove_dir_all(temp);
}

#[test]
fn compiler_preserves_recursive_refs_and_rejects_unsafe_or_unresolved_ops() {
    let mut compiler = BuildCompiler::new(BuildFamily::Pptx);
    compiler
        .push_operation(
            "/",
            None,
            "document",
            "pptx scaffold",
            Map::new(),
            "destination",
        )
        .expect("first op");
    let mut args = Map::new();
    args.insert(
        "slide".to_string(),
        operation_reference("document", "destination.summary.slide").expect("operation ref"),
    );
    args.insert(
        "paragraphs".to_string(),
        json!([{"target": {"$ref": "document.destination.primarySelector"}}]),
    );
    compiler
        .push_operation(
            "/slides/0/textBoxes/0",
            Some("summary"),
            "summary_box",
            "pptx add-textbox",
            args,
            "destination.primarySelector",
        )
        .expect("dependent op");
    let plan = compiler.finish().expect("ordered refs");
    assert_eq!(
        plan.operations_json()[1]["args"],
        json!({
            "paragraphs": [{"target": {"$ref": "document.destination.primarySelector"}}],
            "slide": {"$ref": "document.destination.summary.slide"}
        })
    );

    let mut unresolved = BuildCompiler::new(BuildFamily::Docx);
    let error = unresolved
        .push_operation(
            "/blocks/0",
            None,
            "paragraph",
            "docx paragraphs append",
            Map::from_iter([(
                "after".to_string(),
                json!({"$ref": "future.destination.primarySelector"}),
            )]),
            "destination.primarySelector",
        )
        .expect_err("forward ref must fail at its spec node");
    assert_eq!(error.path, "/blocks/0");
    assert_eq!(error.op_id.as_deref(), Some("paragraph"));
    assert!(error.message.contains("earlier op"), "{error:?}");

    unresolved
        .push_operation(
            "/blocks/0",
            None,
            "paragraph",
            "docx paragraphs append",
            Map::new(),
            "destination.primarySelector",
        )
        .expect("a rejected operation does not reserve its id");

    let mut unsafe_args = BuildCompiler::new(BuildFamily::Xlsx);
    let error = unsafe_args
        .push_operation(
            "/sheets/0",
            None,
            "sheet",
            "xlsx scaffold",
            Map::from_iter([("out".to_string(), json!("book.xlsx"))]),
            "destination.primarySelector",
        )
        .expect_err("session-owned arg must fail");
    assert_eq!(error.path, "/sheets/0");
    assert_eq!(error.op_id.as_deref(), Some("sheet"));
    assert!(error.message.contains("session-owned"), "{error:?}");

    assert_eq!(
        operation_reference("slide:1", "destination.primarySelector").unwrap(),
        json!({"$ref": "slide:1.destination.primarySelector"})
    );
    let error = unsafe_args
        .push_operation(
            "/sheets/1",
            None,
            "sheet:2",
            "xlsx sheets add",
            Map::from_iter([("pretty".to_string(), json!(true))]),
            "destination.primarySelector",
        )
        .expect_err("global formatting arg must remain session-owned");
    assert_eq!(error.path, "/sheets/1");
    assert!(error.message.contains("session-owned"), "{error:?}");
}
