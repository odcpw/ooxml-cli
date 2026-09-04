use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const RECIPE_NAMES: [&str; 11] = [
    "deck-from-scratch",
    "deck-from-template",
    "workbook-report",
    "document-report",
    "macro-workbook",
    "find-replace-package",
    "translate-deck",
    "pivot-report",
    "batch-edit-with-apply",
    "build-from-spec",
    "build-from-markdown",
];

#[test]
fn recipe_catalog_and_individual_lookup_publish_complete_ordered_steps() {
    let catalog = run_json(&["--json", "robot-docs", "recipes"]);
    assert_eq!(catalog["contractVersion"], "ooxml-cli.recipe.v1");
    let recipes = catalog["recipes"].as_array().expect("recipe catalog");
    assert_eq!(recipes.len(), RECIPE_NAMES.len());
    assert_eq!(
        recipes
            .iter()
            .map(|recipe| recipe["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        RECIPE_NAMES
    );
    for recipe in recipes {
        let name = recipe["name"].as_str().unwrap();
        let direct = run_json(&["--json", "robot-docs", "recipe", name]);
        assert_eq!(&direct, recipe, "catalog/detail drift for {name}");
        let steps = recipe["steps"].as_array().expect("ordered recipe steps");
        assert!(!steps.is_empty(), "{name} has no runnable steps");
        assert!(
            recipe["followUps"]
                .as_array()
                .is_some_and(|commands| !commands.is_empty()),
            "{name} has no proof follow-ups"
        );
        assert!(
            recipe["typedMcpTools"]
                .as_array()
                .is_some_and(|tools| !tools.is_empty()),
            "{name} has no typed MCP equivalent"
        );
        for (index, step) in steps.iter().enumerate() {
            assert_eq!(step["index"], index + 1);
            for field in ["command", "purpose", "proofCommand"] {
                assert!(
                    step[field].as_str().is_some_and(|value| !value.is_empty()),
                    "{name} step {} lacks {field}",
                    index + 1
                );
            }
            assert!(
                step["expectedFields"]
                    .as_array()
                    .is_some_and(|fields| !fields.is_empty()),
                "{name} step {} lacks expected fields",
                index + 1
            );
        }
    }
    assert!(
        catalog["recipes"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|recipe| recipe["followUps"].as_array().unwrap())
            .any(|command| command
                .as_str()
                .is_some_and(|command| command.contains(" design-check "))),
        "PPTX recipes must publish design-check follow-up proof"
    );

    let unknown = run(&["--json", "robot-docs", "recipe", "not-a-recipe"]);
    assert_eq!(unknown.status.code(), Some(2));
    assert!(unknown.stdout.is_empty());
    let error: Value = serde_json::from_slice(&unknown.stderr).expect("unknown recipe error");
    assert_eq!(error["error"]["code"], "invalid_args");
    assert!(
        error["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("deck-from-scratch"))
    );
}

#[test]
fn agent_triage_links_recipes_by_detected_family_and_request() {
    let xlsx = run_json(&["--json", "agent-triage", "--file", "quarterly-report.xlsx"]);
    assert_eq!(xlsx["triageInput"]["detectedFamily"], "xlsx");
    assert_eq!(xlsx["recipes"][0]["name"], "workbook-report");
    assert!(recipe_names(&xlsx).contains("pivot-report"));

    let request = run_json(&[
        "--json",
        "agent-triage",
        "--request",
        "translate this presentation from the template",
    ]);
    assert_eq!(request["recipes"][0]["name"], "translate-deck");
    assert!(recipe_names(&request).contains("deck-from-template"));
    for recipe in request["recipes"].as_array().unwrap() {
        assert!(
            recipe["command"]
                .as_str()
                .is_some_and(|command| command.starts_with("ooxml --json robot-docs recipe "))
        );
    }
}

#[test]
fn capabilities_publish_the_same_generated_workflow_contract() {
    let catalog = run_json(&["--json", "robot-docs", "recipes"]);
    let workflows = run_json(&["--json", "capabilities", "--workflows"]);
    assert_eq!(workflows["contractVersion"], "ooxml-cli.recipe.v1");
    assert_eq!(workflows["workflows"], catalog["recipes"]);

    let capabilities = run_json(&["--json", "capabilities"]);
    assert_eq!(capabilities["workflows"], catalog["recipes"]);
    let published_typed_tools = capabilities["mcp"]["typedTools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect::<BTreeSet<_>>();
    for recipe in catalog["recipes"].as_array().unwrap() {
        for tool in recipe["typedMcpTools"].as_array().unwrap() {
            assert!(
                published_typed_tools.contains(tool.as_str().unwrap()),
                "{} references unpublished typed MCP tool {tool}",
                recipe["name"]
            );
        }
    }
    let commands = capabilities["commands"].as_array().unwrap();
    for path in ["ooxml robot-docs recipes", "ooxml robot-docs recipe"] {
        assert!(
            commands.iter().any(|command| command["path"] == path),
            "manifest omitted {path}"
        );
    }
    let triage = commands
        .iter()
        .find(|command| command["path"] == "ooxml agent-triage")
        .unwrap();
    let flags = triage["localFlags"]
        .as_array()
        .unwrap()
        .iter()
        .map(|flag| flag["name"].as_str().unwrap())
        .collect::<BTreeSet<_>>();
    assert!(flags.contains("--file"));
    assert!(flags.contains("--request"));
    let capabilities_command = commands
        .iter()
        .find(|command| command["path"] == "ooxml capabilities")
        .unwrap();
    assert!(
        capabilities_command["localFlags"]
            .as_array()
            .unwrap()
            .iter()
            .any(|flag| flag["name"] == "--workflows")
    );
}

#[test]
fn readme_and_skill_recipe_sections_are_byte_exact_binary_output() {
    let output = run(&["robot-docs", "recipes", "--format", "text"]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert!(
        !output.stdout.contains(&b'\r'),
        "binary emitted non-LF text"
    );

    for relative in ["README.md", "skills/ooxml/SKILL.md"] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
        let document = std::fs::read(&path).unwrap();
        assert!(!document.contains(&b'\r'), "{relative} is not LF-only");
        assert_eq!(
            generated_recipe_section(&document),
            output.stdout.as_slice(),
            "{relative} drifted; run make docs-recipes"
        );
    }
}

#[test]
fn every_published_recipe_runs_end_to_end_and_returns_expected_fields() {
    let catalog = run_json(&["--json", "robot-docs", "recipes"]);
    let recipes = catalog["recipes"].as_array().unwrap();
    assert_eq!(recipes.len(), RECIPE_NAMES.len());
    for recipe in recipes {
        run_recipe(recipe);
    }
}

fn run_recipe(recipe: &Value) {
    let name = recipe["name"].as_str().unwrap();
    let temp = temp_dir(name);
    let replacements = recipe_replacements(name, &temp);
    if name == "batch-edit-with-apply" {
        let operations = json!([{
            "id": "set_cell",
            "command": "xlsx cells set",
            "args": {"sheet": "Sheet1", "cell": "A1", "value": "Batched"},
        }]);
        std::fs::write(
            replacements["<ops.json>"].as_path(),
            format!("{}\n", serde_json::to_string_pretty(&operations).unwrap()),
        )
        .expect("write recipe ops");
    }

    for step in recipe["steps"].as_array().unwrap() {
        let command = substitute(step["command"].as_str().unwrap(), &replacements);
        let (output, captured) = run_recipe_command(&command);
        assert!(
            output.status.success(),
            "{name} command failed: {command}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let value: Value = serde_json::from_slice(&output.stdout)
            .unwrap_or_else(|error| panic!("{name} returned non-JSON for {command}: {error}"));
        for pointer in step["expectedFields"].as_array().unwrap() {
            let pointer = pointer.as_str().unwrap();
            assert!(
                value.pointer(pointer).is_some(),
                "{name} command {command} omitted expected field {pointer}: {value:#}"
            );
        }
        if let Some(path) = captured {
            std::fs::write(path, &output.stdout).expect("capture recipe stdout");
        }

        let proof = substitute(step["proofCommand"].as_str().unwrap(), &replacements);
        let (proof_output, captured) = run_recipe_command(&proof);
        assert!(captured.is_none(), "proof commands cannot redirect output");
        assert!(
            proof_output.status.success(),
            "{name} proof failed: {proof}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&proof_output.stdout),
            String::from_utf8_lossy(&proof_output.stderr)
        );
        let proof_value: Value = serde_json::from_slice(&proof_output.stdout)
            .unwrap_or_else(|error| panic!("{name} proof returned non-JSON: {error}"));
        if proof.contains(" check ") {
            assert_eq!(proof_value["summary"]["errors"], 0, "{name}: {proof_value}");
        } else {
            assert_eq!(proof_value["valid"], true, "{name}: {proof_value}");
        }
    }
    for follow_up in recipe["followUps"].as_array().unwrap() {
        let command = substitute(follow_up.as_str().unwrap(), &replacements);
        let (output, captured) = run_recipe_command(&command);
        assert!(captured.is_none(), "follow-ups cannot redirect output");
        assert!(
            output.status.success(),
            "{name} follow-up failed: {command}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let value: Value = serde_json::from_slice(&output.stdout)
            .unwrap_or_else(|error| panic!("{name} follow-up returned non-JSON: {error}"));
        if command.contains(" check ") {
            assert_eq!(value["summary"]["errors"], 0, "{name}: {value}");
        }
    }
    let _ = std::fs::remove_dir_all(temp);
}

fn recipe_replacements(name: &str, temp: &Path) -> BTreeMap<&'static str, PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut values = BTreeMap::from([
        (
            "<template.pptx>",
            root.join("testdata/pptx/multi-layout/presentation.pptx"),
        ),
        (
            "<workbook-spec.json>",
            root.join("testdata/xlsx/build-spec/sales.json"),
        ),
        (
            "<presentation-spec.json>",
            root.join("testdata/pptx/build-spec/q3-review.json"),
        ),
        (
            "<document-spec.json>",
            root.join("testdata/docx/build-spec/quarterly-report.json"),
        ),
        (
            "<module.bas>",
            root.join("testdata/golden/vba-authoring/xlsx-standard/AgentSmoke.bas"),
        ),
        ("<deck.md>", root.join("testdata/markdown/q3-review.md")),
        (
            "<document.md>",
            root.join("testdata/markdown/quarterly-report.md"),
        ),
        ("<output.pptx>", temp.join("output.pptx")),
        ("<output.xlsx>", temp.join("output.xlsx")),
        ("<output.docx>", temp.join("output.docx")),
        ("<output.xlsm>", temp.join("output.xlsm")),
        ("<base.xlsx>", temp.join("base.xlsx")),
        ("<manifest.json>", temp.join("manifest.json")),
        ("<ops.json>", temp.join("ops.json")),
        (
            "<input-file>",
            root.join("testdata/xlsx/minimal-workbook/workbook.xlsx"),
        ),
        ("<output-file>", temp.join("output.xlsx")),
    ]);
    values.insert(
        "<input.xlsx>",
        if name == "pivot-report" {
            root.join("testdata/xlsx/outline-table/workbook.xlsx")
        } else {
            root.join("testdata/xlsx/minimal-workbook/workbook.xlsx")
        },
    );
    values.insert(
        "<input.pptx>",
        root.join("testdata/pptx/minimal-title/presentation.pptx"),
    );
    values
}

fn substitute(command: &str, replacements: &BTreeMap<&str, PathBuf>) -> String {
    replacements
        .iter()
        .fold(command.to_string(), |command, (placeholder, value)| {
            command.replace(placeholder, value.to_string_lossy().as_ref())
        })
}

fn run_recipe_command(command: &str) -> (Output, Option<PathBuf>) {
    let mut words = command.split_whitespace().collect::<Vec<_>>();
    assert_eq!(words.first(), Some(&"ooxml"), "recipe command: {command}");
    words.remove(0);
    let capture = words.iter().position(|word| *word == ">").map(|index| {
        assert_eq!(
            index + 2,
            words.len(),
            "redirection must be last: {command}"
        );
        let path = PathBuf::from(words[index + 1]);
        words.truncate(index);
        path
    });
    let output = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args(words)
        .output()
        .expect("run recipe command");
    (output, capture)
}

fn recipe_names(value: &Value) -> BTreeSet<&str> {
    value["recipes"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|recipe| recipe["name"].as_str())
        .collect()
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args(args)
        .output()
        .expect("run ooxml")
}

fn run_json(args: &[&str]) -> Value {
    let output = run(args);
    assert!(
        output.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "{args:?}");
    serde_json::from_slice(&output.stdout).expect("ooxml JSON output")
}

fn generated_recipe_section(document: &[u8]) -> &[u8] {
    const START: &[u8] = b"<!-- BEGIN GENERATED OOXML RECIPES -->\n";
    const END: &[u8] = b"<!-- END GENERATED OOXML RECIPES -->";
    let start = document
        .windows(START.len())
        .position(|window| window == START)
        .expect("generated recipe start marker")
        + START.len();
    let end = document[start..]
        .windows(END.len())
        .position(|window| window == END)
        .expect("generated recipe end marker")
        + start;
    &document[start..end]
}

fn temp_dir(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("target"));
    let path = target
        .join("robot-recipes")
        .join(format!("{label}-{}-{nonce}", std::process::id()));
    std::fs::create_dir_all(&path).expect("create recipe temp directory");
    path
}
