use serde::Serialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const COMMAND_GOLDEN: &str =
    include_str!("../testdata/golden/fresh-agent-simulation/command-sequence.json");

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Step {
    task: &'static str,
    intent: &'static str,
    argv: Vec<String>,
    discovered_from: &'static str,
}

struct PublicSurfaces {
    root_help: String,
    readme: &'static str,
    skill: &'static str,
    capabilities: Value,
    command_lines: BTreeSet<String>,
}

#[test]
fn fresh_agent_discovers_and_completes_the_canonical_authoring_tasks() {
    let surfaces = PublicSurfaces::read();
    let plan = discovered_plan(&surfaces);
    assert_command_golden(&plan);

    let workspace = artifact_root();
    fs::create_dir_all(&workspace).expect("create fresh-agent workspace");
    let paths = prepare_inputs(&workspace);
    let mut transcript = Vec::new();

    for (index, step) in plan.iter().enumerate() {
        let argv = expand_argv(&step.argv, &paths);
        let output = run_argv(&argv);
        transcript.push(transcript_row(index + 1, step, &output));
        let transcript_path = write_transcript(&workspace, &transcript);
        assert!(
            output.status.success(),
            "fresh-agent step failed: {}\nstdout:\n{}\nstderr:\n{}\ntranscript: {}",
            display_argv(&argv),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
            transcript_path.display()
        );
        assert_step_contract(step, &argv, &output, &paths);
    }
}

impl PublicSurfaces {
    fn read() -> Self {
        let root_help = run(&["--help"]);
        assert!(root_help.status.success(), "root help failed");
        let root_help = String::from_utf8(root_help.stdout).expect("UTF-8 root help");
        assert!(
            root_help.contains("capabilities"),
            "root help hides capabilities"
        );

        let capability_output = run(&["--json", "capabilities"]);
        assert!(
            capability_output.status.success(),
            "capabilities failed: {}",
            String::from_utf8_lossy(&capability_output.stderr)
        );
        let capabilities: Value =
            serde_json::from_slice(&capability_output.stdout).expect("capabilities JSON");
        let readme = include_str!("../README.md");
        let skill = include_str!("../skills/ooxml/SKILL.md");
        let mut command_lines = documented_commands(readme);
        command_lines.extend(documented_commands(skill));
        collect_capability_commands(&capabilities, &mut command_lines);
        Self {
            root_help,
            readme,
            skill,
            capabilities,
            command_lines,
        }
    }

    fn documented_template(&self, required: &[&str]) -> Vec<String> {
        let command = self
            .command_lines
            .iter()
            .find(|command| required.iter().all(|token| command.contains(token)))
            .unwrap_or_else(|| {
                panic!(
                    "public README/SKILL/help/capabilities surfaces do not document a command containing {required:?}"
                )
            });
        assert!(
            self.readme.contains(command),
            "README omits discovered recipe command: {command}"
        );
        assert!(
            self.skill.contains(command),
            "skills/ooxml/SKILL.md omits discovered recipe command: {command}"
        );
        command.split_whitespace().map(str::to_string).collect()
    }

    fn require_leaf(&self, path: &str, flags: &[&str]) {
        let command = self.capabilities["commands"]
            .as_array()
            .and_then(|commands| commands.iter().find(|command| command["path"] == path))
            .unwrap_or_else(|| panic!("capabilities omit {path}"));
        let published = command["localFlags"]
            .as_array()
            .or_else(|| command["flags"].as_array())
            .unwrap_or_else(|| panic!("capabilities omit flags for {path}: {command}"))
            .iter()
            .filter_map(|flag| flag["name"].as_str())
            .collect::<BTreeSet<_>>();
        for flag in flags {
            assert!(
                published.contains(flag),
                "{path} hides {flag} in capabilities"
            );
        }
    }
}

fn discovered_plan(surfaces: &PublicSurfaces) -> Vec<Step> {
    assert!(surfaces.root_help.contains("--json"));
    surfaces.require_leaf(
        "ooxml pptx build",
        &["--spec", "--from-markdown", "--out", "--check", "--force"],
    );
    surfaces.require_leaf(
        "ooxml xlsx build",
        &["--spec", "--out", "--check", "--force"],
    );
    surfaces.require_leaf(
        "ooxml docx build",
        &["--spec", "--out", "--check", "--force"],
    );
    surfaces.require_leaf(
        "ooxml pptx replace text",
        &["--slide", "--target", "--text", "--out"],
    );
    surfaces.require_leaf("ooxml check", &["--openxml-sdk", "--fail-on"]);
    surfaces.require_leaf("ooxml design-check", &[]);
    surfaces.require_leaf("ooxml render", &["--out", "--dpi"]);

    let mut pptx_spec = surfaces.documented_template(&[
        "pptx build",
        "--spec <presentation-spec.json>",
        "--out <output.pptx>",
    ]);
    replace_arg(
        &mut pptx_spec,
        "<presentation-spec.json>",
        "<branded-spec.json>",
    );
    replace_arg(&mut pptx_spec, "<output.pptx>", "<branded-deck.pptx>");

    let mut xlsx_spec = surfaces.documented_template(&[
        "xlsx build",
        "--spec <workbook-spec.json>",
        "--out <output.xlsx>",
    ]);
    replace_arg(&mut xlsx_spec, "<output.xlsx>", "<sales.xlsx>");

    let mut docx_spec = surfaces.documented_template(&[
        "docx build",
        "--spec <document-spec.json>",
        "--out <output.docx>",
    ]);
    replace_arg(&mut docx_spec, "<output.docx>", "<report.docx>");

    let mut markdown = surfaces.documented_template(&[
        "pptx build",
        "--from-markdown <deck.md>",
        "--out <output.pptx>",
    ]);
    replace_arg(&mut markdown, "<output.pptx>", "<markdown-deck.pptx>");

    vec![
        step(
            "discovery",
            "Ask the binary for its top-level command menu.",
            &["ooxml", "--help"],
            "ooxml --help",
        ),
        step(
            "discovery",
            "Read the machine-readable command and flag inventory.",
            &["ooxml", "--json", "capabilities"],
            "ooxml --json capabilities",
        ),
        owned_step(
            "branded-deck-from-spec",
            "Use the documented spec builder on the supplied branded deck spec.",
            pptx_spec,
            "README.md + skills/ooxml/SKILL.md build-from-spec recipe",
        ),
        proof_step("branded-deck-from-spec", "<branded-deck.pptx>"),
        owned_step(
            "workbook-with-chart",
            "Use the documented workbook recipe on the supplied chart spec.",
            xlsx_spec,
            "README.md + skills/ooxml/SKILL.md workbook recipe",
        ),
        proof_step("workbook-with-chart", "<sales.xlsx>"),
        owned_step(
            "report-document",
            "Use the documented document recipe on the supplied report spec.",
            docx_spec,
            "README.md + skills/ooxml/SKILL.md document recipe",
        ),
        proof_step("report-document", "<report.docx>"),
        owned_step(
            "markdown-sourced-deck",
            "Use the documented Markdown builder without authoring intermediate JSON.",
            markdown,
            "README.md + skills/ooxml/SKILL.md build-from-markdown recipe",
        ),
        proof_step("markdown-sourced-deck", "<markdown-deck.pptx>"),
        step(
            "edit-and-check-deck",
            "Use the natural PPTX replace text vocabulary advertised by capabilities.",
            &[
                "ooxml",
                "--json",
                "pptx",
                "replace",
                "text",
                "<branded-deck.pptx>",
                "--slide",
                "1",
                "--target",
                "title",
                "--text",
                "Q3 review — fresh agent",
                "--out",
                "<edited-deck.pptx>",
            ],
            "capabilities pptx inspect then edit workflow + command flags",
        ),
        proof_step("edit-and-check-deck", "<edited-deck.pptx>"),
        step(
            "design-check-and-render-deck",
            "Run the advertised objective design review before rendering.",
            &["ooxml", "--json", "design-check", "<edited-deck.pptx>"],
            "capabilities command ooxml design-check",
        ),
        step(
            "design-check-and-render-deck",
            "Render the reviewed presentation through the advertised shared renderer.",
            &[
                "ooxml",
                "--json",
                "render",
                "<edited-deck.pptx>",
                "--out",
                "<render-dir>",
                "--dpi",
                "96",
            ],
            "capabilities command ooxml render",
        ),
    ]
}

fn proof_step(task: &'static str, package: &str) -> Step {
    owned_step(
        task,
        "Run the advertised one-call strict and Open XML SDK package proof.",
        [
            "ooxml",
            "--json",
            "check",
            package,
            "--openxml-sdk",
            "auto",
            "--fail-on",
            "error",
        ]
        .into_iter()
        .map(str::to_string)
        .collect(),
        "capabilities command ooxml check",
    )
}

fn step(
    task: &'static str,
    intent: &'static str,
    argv: &[&str],
    discovered_from: &'static str,
) -> Step {
    owned_step(
        task,
        intent,
        argv.iter().map(|arg| (*arg).to_string()).collect(),
        discovered_from,
    )
}

fn owned_step(
    task: &'static str,
    intent: &'static str,
    argv: Vec<String>,
    discovered_from: &'static str,
) -> Step {
    Step {
        task,
        intent,
        argv,
        discovered_from,
    }
}

fn documented_commands(document: &str) -> BTreeSet<String> {
    document
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let start = line.find("ooxml ")?;
            let command = line[start..].trim_matches('`').trim().to_string();
            (!command.contains('|') && !command.ends_with(':')).then_some(command)
        })
        .collect()
}

fn collect_capability_commands(value: &Value, commands: &mut BTreeSet<String>) {
    match value {
        Value::String(value) if value.starts_with("ooxml ") => {
            commands.insert(value.clone());
        }
        Value::Array(values) => {
            for value in values {
                collect_capability_commands(value, commands);
            }
        }
        Value::Object(values) => {
            for value in values.values() {
                collect_capability_commands(value, commands);
            }
        }
        _ => {}
    }
}

fn replace_arg(argv: &mut [String], from: &str, to: &str) {
    let Some(index) = argv.iter().position(|arg| arg == from) else {
        panic!("documented command omits {from}: {argv:?}");
    };
    argv[index] = to.to_string();
}

fn assert_command_golden(plan: &[Step]) {
    let actual = serde_json::to_vec_pretty(&json!({
        "contractVersion": "ooxml-cli.fresh-agent-sequence.v1",
        "steps": plan,
    }))
    .expect("serialize command sequence");
    let expected: Value = serde_json::from_str(COMMAND_GOLDEN).expect("command golden JSON");
    let actual_value: Value = serde_json::from_slice(&actual).expect("actual command JSON");
    assert_eq!(
        actual_value, expected,
        "fresh-agent command sequence drifted; update the golden only after reviewing the public discovery surfaces"
    );
}

fn prepare_inputs(workspace: &Path) -> BTreeMap<&'static str, PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut branded: Value = serde_json::from_slice(
        &fs::read(root.join("testdata/pptx/build-spec/q3-review.json"))
            .expect("read PPTX build spec"),
    )
    .expect("PPTX build spec JSON");
    branded
        .as_object_mut()
        .expect("PPTX build spec object")
        .remove("theme");
    branded["brand"] = json!({
        "path": root.join("testdata/brand/northwind.json").to_string_lossy(),
    });
    branded["slides"][3]["images"][0]["path"] =
        json!(root.join("testdata/test_image.png").to_string_lossy());
    let branded_spec = workspace.join("branded-deck.json");
    fs::write(
        &branded_spec,
        serde_json::to_vec_pretty(&branded).expect("serialize branded deck spec"),
    )
    .expect("write branded deck spec");

    BTreeMap::from([
        ("<branded-spec.json>", branded_spec),
        (
            "<workbook-spec.json>",
            root.join("testdata/xlsx/build-spec/sales.json"),
        ),
        (
            "<document-spec.json>",
            root.join("testdata/docx/build-spec/quarterly-report.json"),
        ),
        ("<deck.md>", root.join("testdata/markdown/q3-review.md")),
        ("<branded-deck.pptx>", workspace.join("branded-deck.pptx")),
        ("<sales.xlsx>", workspace.join("sales.xlsx")),
        ("<report.docx>", workspace.join("report.docx")),
        ("<markdown-deck.pptx>", workspace.join("markdown-deck.pptx")),
        ("<edited-deck.pptx>", workspace.join("edited-deck.pptx")),
        ("<render-dir>", workspace.join("rendered")),
    ])
}

fn expand_argv(argv: &[String], paths: &BTreeMap<&str, PathBuf>) -> Vec<String> {
    argv.iter()
        .map(|arg| {
            paths
                .get(arg.as_str())
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_else(|| arg.clone())
        })
        .collect()
}

fn assert_step_contract(
    step: &Step,
    argv: &[String],
    output: &Output,
    paths: &BTreeMap<&str, PathBuf>,
) {
    if argv.get(1).is_some_and(|arg| arg == "--help") {
        assert!(String::from_utf8_lossy(&output.stdout).contains("capabilities"));
        return;
    }
    let value: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "{} returned non-JSON: {error}\n{}",
            display_argv(argv),
            String::from_utf8_lossy(&output.stdout)
        )
    });
    if argv.get(2).is_some_and(|arg| arg == "capabilities") {
        assert!(
            value["commands"]
                .as_array()
                .is_some_and(|commands| !commands.is_empty())
        );
        return;
    }
    if argv.iter().any(|arg| arg == "build") {
        assert_eq!(value["validated"], true, "{value}");
        assert_eq!(value["check"]["summary"]["errors"], 0, "{value}");
    } else if argv.get(2).is_some_and(|arg| arg == "check") {
        assert_eq!(value["summary"]["errors"], 0, "{value}");
        assert_eq!(value["checks"]["strict"], "passed", "{value}");
        if sdk_available() {
            assert_eq!(value["checks"]["schema"], "passed", "{value}");
        } else {
            assert_eq!(value["checks"]["schema"], "skipped", "{value}");
            assert!(!proof_required("OOXML_REQUIRE_OPENXML_SDK"));
        }
    } else if argv.get(3).is_some_and(|arg| arg == "replace")
        && argv.get(4).is_some_and(|arg| arg == "text")
    {
        assert_eq!(value["newText"], "Q3 review — fresh agent", "{value}");
    } else if argv.get(2).is_some_and(|arg| arg == "design-check") {
        assert_eq!(value["summary"]["errors"], 0, "{value}");
    } else if argv.get(2).is_some_and(|arg| arg == "render") {
        if value["status"] == "skipped" {
            assert!(!render_required(), "render was required: {value}");
        } else {
            assert_eq!(value["status"], "ok", "{value}");
            assert_eq!(value["slides"].as_array().map(Vec::len), Some(5), "{value}");
        }
    }

    if let Some(package) = produced_package(step, paths) {
        assert!(
            package.is_file(),
            "{} did not produce {}",
            step.task,
            package.display()
        );
        assert_strict_and_sdk(package);
    }
}

fn produced_package<'a>(step: &Step, paths: &'a BTreeMap<&str, PathBuf>) -> Option<&'a Path> {
    let placeholder = match step.task {
        "branded-deck-from-spec" if step.argv.iter().any(|arg| arg == "build") => {
            "<branded-deck.pptx>"
        }
        "workbook-with-chart" if step.argv.iter().any(|arg| arg == "build") => "<sales.xlsx>",
        "report-document" if step.argv.iter().any(|arg| arg == "build") => "<report.docx>",
        "markdown-sourced-deck" if step.argv.iter().any(|arg| arg == "build") => {
            "<markdown-deck.pptx>"
        }
        "edit-and-check-deck" if step.argv.iter().any(|arg| arg == "replace") => {
            "<edited-deck.pptx>"
        }
        _ => return None,
    };
    Some(paths[placeholder].as_path())
}

fn assert_strict_and_sdk(package: &Path) {
    let package_text = package.to_string_lossy();
    let strict = run(&["--json", "validate", package_text.as_ref(), "--strict"]);
    assert!(
        strict.status.success(),
        "strict validation failed for {}: {}",
        package.display(),
        String::from_utf8_lossy(&strict.stderr)
    );
    let value: Value = serde_json::from_slice(&strict.stdout).expect("strict validation JSON");
    assert_eq!(value["valid"], true, "{value}");

    if let Some((dotnet, validator)) = sdk_tools() {
        let output = Command::new(dotnet)
            .arg(validator)
            .arg(package)
            .output()
            .expect("run Open XML SDK validator");
        assert!(
            output.status.success(),
            "SDK validation failed for {}:\n{}{}",
            package.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    } else {
        assert!(!proof_required("OOXML_REQUIRE_OPENXML_SDK"));
    }
}

fn sdk_available() -> bool {
    sdk_tools().is_some()
}

fn sdk_tools() -> Option<(PathBuf, PathBuf)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let validator = root.join("tools/openxml-validator/bin/Release/net8.0/openxml-validator.dll");
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })?;
    let dotnet = PathBuf::from(home).join("dotnet").join(if cfg!(windows) {
        "dotnet.exe"
    } else {
        "dotnet"
    });
    (validator.is_file() && dotnet.is_file()).then_some((dotnet, validator))
}

fn transcript_row(index: usize, step: &Step, output: &Output) -> Value {
    json!({
        "task": step.task,
        "step": index,
        "intent": step.intent,
        "invocation": display_argv(&step.argv),
        "argv": step.argv,
        "exitCode": output.status.code(),
        "outcome": if output.status.success() { "success" } else { "error" },
        "stdout": truncate(&String::from_utf8_lossy(&output.stdout)),
        "stderr": truncate(&String::from_utf8_lossy(&output.stderr)),
    })
}

fn write_transcript(workspace: &Path, transcript: &[Value]) -> PathBuf {
    let path = workspace.join("fresh-agent-transcript.json");
    fs::write(
        &path,
        serde_json::to_vec_pretty(transcript).expect("serialize transcript"),
    )
    .expect("write transcript");
    path
}

fn truncate(value: &str) -> String {
    const LIMIT: usize = 4096;
    if value.len() <= LIMIT {
        return value.to_string();
    }
    let mut end = LIMIT;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}... [truncated]", &value[..end])
}

fn artifact_root() -> PathBuf {
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("target"));
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    target.join(format!(
        "ooxml-fresh-agent-simulation-{}-{nonce}",
        std::process::id()
    ))
}

fn run_argv(argv: &[String]) -> Output {
    assert_eq!(argv.first().map(String::as_str), Some("ooxml"));
    Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args(&argv[1..])
        .output()
        .unwrap_or_else(|error| panic!("run {}: {error}", display_argv(argv)))
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args(args)
        .output()
        .expect("run ooxml")
}

fn display_argv(argv: &[String]) -> String {
    argv.iter()
        .map(|arg| {
            if arg.contains(char::is_whitespace) {
                format!("{arg:?}")
            } else {
                arg.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn proof_required(name: &str) -> bool {
    std::env::var(name).is_ok_and(|value| value == "1" || value.eq_ignore_ascii_case("true"))
}

fn render_required() -> bool {
    ["OOXML_REQUIRE_RENDER", "OOXML_REQUIRE_LIBREOFFICE"]
        .into_iter()
        .any(proof_required)
}
