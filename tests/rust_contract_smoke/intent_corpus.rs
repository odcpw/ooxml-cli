use super::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::io::BufWriter;

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct FrozenProcessCase {
    name: String,
    category: String,
    argv: Vec<String>,
    status: i32,
    stdout: String,
    stderr: String,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct FrozenHelpCorpus {
    schema_version: u32,
    group_count: usize,
    leaf_count: usize,
    group_topic_rows: usize,
    alias_owner_records: usize,
    alias_argv_count: usize,
    cases: Vec<FrozenProcessCase>,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct FrozenProcessMatrix {
    schema_version: u32,
    cases: Vec<FrozenProcessCase>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IntentManifest {
    commands: Vec<IntentCommand>,
    #[serde(default)]
    command_aliases: Vec<IntentCommandAliasRecord>,
    #[serde(default)]
    flag_aliases: Vec<IntentAliasRecord>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IntentCommand {
    path: String,
    #[serde(default)]
    aliases: Vec<String>,
    #[serde(default)]
    local_flags: Vec<IntentFlag>,
    #[serde(default)]
    op_ineligible_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IntentFlag {
    name: String,
    #[serde(rename = "type")]
    flag_type: String,
    #[serde(default)]
    aliases: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IntentAliasRecord {
    path: String,
    alias: String,
    canonical_flags: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct IntentCommandAliasRecord {
    alias: String,
    canonical_command: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GeneratedIntentCase {
    id: String,
    category: String,
    command_path: String,
    argv: Vec<String>,
    bad_token: String,
    canonical_token: String,
    expected_alias: bool,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
enum IntentOutcome {
    SilentFail,
    UselessError,
    UsefulHint,
    Inferred,
}

impl IntentOutcome {
    fn label(self) -> &'static str {
        match self {
            Self::SilentFail => "silent-fail",
            Self::UselessError => "useless-error",
            Self::UsefulHint => "useful-hint",
            Self::Inferred => "inferred",
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct IntentObservation<'a> {
    case: &'a GeneratedIntentCase,
    outcome: IntentOutcome,
    exit_code: i32,
    error_code: Option<String>,
    message: Option<String>,
    did_you_mean: Vec<String>,
    valid_flag_count: usize,
}

fn refresh_frozen_case(case: &mut FrozenProcessCase) {
    let argv = case.argv.iter().map(String::as_str).collect::<Vec<_>>();
    let actual = run_ooxml_process(&argv);
    case.status = actual.code;
    case.stdout = String::from_utf8(actual.stdout).expect("stdout UTF-8");
    case.stderr = String::from_utf8(actual.stderr).expect("stderr UTF-8");
}

fn sync_frozen_help_capability_cases(help: &mut FrozenHelpCorpus, manifest: &IntentManifest) {
    let existing = help
        .cases
        .iter()
        .filter(|case| matches!(case.category.as_str(), "canonical-group" | "canonical-leaf"))
        .map(|case| case.argv.clone())
        .collect::<BTreeSet<_>>();
    let mut groups = Vec::new();
    let mut leaves = Vec::new();
    for command in &manifest.commands {
        let topic = command
            .path
            .split_whitespace()
            .skip_while(|token| *token == "ooxml")
            .map(str::to_string)
            .collect::<Vec<_>>();
        let argv = std::iter::once("help".to_string())
            .chain(topic.iter().cloned())
            .collect::<Vec<_>>();
        if existing.contains(&argv) {
            continue;
        }
        let group = command.op_ineligible_reason.as_deref()
            == Some("it is a command group, not a leaf mutation command");
        let case = FrozenProcessCase {
            name: format!(
                "{}:{}",
                if group { "group" } else { "leaf" },
                topic.join(" ")
            ),
            category: if group {
                "canonical-group".to_string()
            } else {
                "canonical-leaf".to_string()
            },
            argv,
            status: 0,
            stdout: String::new(),
            stderr: String::new(),
        };
        if group {
            groups.push(case);
        } else {
            leaves.push(case);
        }
    }

    let group_insert = help
        .cases
        .iter()
        .position(|case| case.category == "canonical-leaf")
        .unwrap_or(help.cases.len());
    help.cases.splice(group_insert..group_insert, groups);
    let leaf_insert = help
        .cases
        .iter()
        .position(|case| {
            !matches!(
                case.category.as_str(),
                "root" | "canonical-group" | "canonical-leaf"
            )
        })
        .unwrap_or(help.cases.len());
    help.cases.splice(leaf_insert..leaf_insert, leaves);
    help.group_count = help
        .cases
        .iter()
        .filter(|case| case.category == "canonical-group")
        .count();
    help.leaf_count = help
        .cases
        .iter()
        .filter(|case| case.category == "canonical-leaf")
        .count();
}

fn write_reviewed_golden(path: &str, document: &impl Serialize) {
    let mut bytes = serde_json::to_vec_pretty(document).expect("serialize process corpus");
    bytes.push(b'\n');
    std::fs::write(path, bytes).expect("update reviewed invalid-args corpus");
}

fn explicit_json_error(args: &[&str]) -> Value {
    let output = run_ooxml_process(args);
    assert_eq!(output.code, 2, "invalid invocation: {args:?}");
    assert!(
        output.stderr.is_empty(),
        "explicit JSON diagnostics must not contaminate stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).expect("JSON stdout is UTF-8");
    assert_eq!(text.lines().count(), 1, "one JSON object: {text:?}");
    serde_json::from_str(&text).expect("structured invalid-args JSON")
}

fn live_intent_manifest() -> IntentManifest {
    let output = run_ooxml_process(&["--json", "capabilities"]);
    assert_eq!(output.code, 0, "live capabilities exit");
    assert!(
        output.stderr.is_empty(),
        "live capabilities stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("live capabilities contract")
}

fn explicit_json_success(args: &[String]) -> Value {
    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    let output = run_ooxml_process(&refs);
    assert_eq!(
        output.code,
        0,
        "successful invocation {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "explicit JSON diagnostics must not contaminate stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).expect("JSON stdout is UTF-8");
    assert_eq!(text.lines().count(), 1, "one JSON object: {text:?}");
    serde_json::from_str(&text).expect("structured success JSON")
}

fn assert_intent_output_strict_valid(path: &str, label: &str) {
    let (code, stdout, stderr) = run_ooxml(&["--json", "validate", "--strict", path]);
    assert_eq!(code, 0, "{label} strict validation exit: {stderr:?}");
    assert_eq!(stderr, None, "{label} strict validation stderr");
    assert_eq!(
        stdout.expect("strict validation JSON")["valid"],
        true,
        "{label}"
    );
}

fn assert_alias_readback(args: &[&str], expected: Value) -> Value {
    let args = args
        .iter()
        .map(|value| (*value).to_string())
        .collect::<Vec<_>>();
    let value = explicit_json_success(&args);
    assert_eq!(value["aliasesApplied"], expected, "{args:?}: {value:?}");
    value
}

fn remove_alias_readbacks(value: &mut Value) {
    match value {
        Value::Object(object) => {
            object.remove("aliasesApplied");
            for child in object.values_mut() {
                remove_alias_readbacks(child);
            }
        }
        Value::Array(array) => {
            for child in array {
                remove_alias_readbacks(child);
            }
        }
        _ => {}
    }
}

fn replace_flag(args: &[String], canonical: &str, alias: &str) -> Vec<String> {
    let mut replaced = args.to_vec();
    let index = replaced
        .iter()
        .position(|arg| arg == canonical)
        .unwrap_or_else(|| panic!("canonical invocation omitted {canonical}: {args:?}"));
    replaced[index] = alias.to_string();
    replaced
}

fn alias_base_invocation(
    path: &str,
    table_data: &str,
    clip: &str,
    media_deck: &str,
    media_shape: &str,
    block_hash: &str,
) -> Vec<String> {
    let args: &[&str] = match path {
        "ooxml pptx slides import-slide" => &[
            "pptx",
            "slides",
            "import-slide",
            "testdata/pptx/multi-layout/presentation.pptx",
            "--source",
            "testdata/pptx/multi-layout/presentation.pptx",
            "--slide",
            "1",
            "--insert-after",
            "1",
            "--dry-run",
        ],
        "ooxml pptx clone-slide" => &[
            "pptx",
            "clone-slide",
            "testdata/pptx/multi-layout/presentation.pptx",
            "--slide",
            "1",
            "--insert-after",
            "1",
            "--dry-run",
        ],
        "ooxml pptx new-slide-from-layout" => &[
            "pptx",
            "new-slide-from-layout",
            "testdata/pptx/multi-layout/presentation.pptx",
            "--layout",
            "Title and Content",
            "--insert-after",
            "1",
            "--dry-run",
        ],
        "ooxml docx paragraphs insert" => &[
            "docx",
            "paragraphs",
            "insert",
            "testdata/docx/styled-headings/document.docx",
            "--insert-after",
            "0",
            "--text",
            "Alias paragraph",
            "--dry-run",
        ],
        "ooxml docx images insert" => &[
            "docx",
            "images",
            "insert",
            "testdata/docx/styled-headings/document.docx",
            "--after",
            "0",
            "--file",
            "testdata/test_image.png",
            "--width",
            "914400",
            "--height",
            "914400",
            "--dry-run",
        ],
        "ooxml docx paragraphs set" => &[
            "docx",
            "paragraphs",
            "set",
            "testdata/docx/styled-headings/document.docx",
            "--index",
            "1",
            "--text",
            "Alias paragraph",
            "--dry-run",
        ],
        "ooxml docx paragraphs clear" => &[
            "docx",
            "paragraphs",
            "clear",
            "testdata/docx/styled-headings/document.docx",
            "--index",
            "1",
            "--dry-run",
        ],
        "ooxml docx styles apply" => &[
            "docx",
            "styles",
            "apply",
            "testdata/docx/apply-styles/document.docx",
            "--index",
            "1",
            "--target",
            "paragraph",
            "--style",
            "Heading2",
            "--dry-run",
        ],
        "ooxml docx blocks" => &[
            "docx",
            "blocks",
            "testdata/docx/styled-headings/document.docx",
            "--block",
            "1",
        ],
        "ooxml docx blocks replace" => &[
            "docx",
            "blocks",
            "replace",
            "testdata/docx/styled-headings/document.docx",
            "--block",
            "1",
            "--expect-hash",
            block_hash,
            "--text",
            "Alias replacement",
            "--dry-run",
        ],
        "ooxml docx blocks delete" => &[
            "docx",
            "blocks",
            "delete",
            "testdata/docx/styled-headings/document.docx",
            "--block",
            "1",
            "--expect-hash",
            block_hash,
            "--dry-run",
        ],
        "ooxml docx blocks insert-after" => &[
            "docx",
            "blocks",
            "insert-after",
            "testdata/docx/styled-headings/document.docx",
            "--block",
            "1",
            "--expect-hash",
            block_hash,
            "--text",
            "Alias insertion",
            "--dry-run",
        ],
        "ooxml pptx place table" => &[
            "pptx",
            "place",
            "table",
            "testdata/pptx/minimal-title/presentation.pptx",
            "--slide",
            "1",
            "--data",
            table_data,
            "--format",
            "json",
            "--x",
            "0",
            "--y",
            "0",
            "--cx",
            "2000000",
            "--dry-run",
        ],
        "ooxml pptx media add" => &[
            "pptx",
            "media",
            "add",
            "testdata/pptx/minimal-title/presentation.pptx",
            "--slide",
            "1",
            "--file",
            clip,
            "--dry-run",
        ],
        "ooxml pptx media replace" => &[
            "pptx",
            "media",
            "replace",
            media_deck,
            "--slide",
            "1",
            "--shape",
            media_shape,
            "--file",
            clip,
            "--dry-run",
        ],
        "ooxml xlsx colwidths show" => &[
            "xlsx",
            "colwidths",
            "show",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
            "--sheet",
            "Sheet1",
            "--range",
            "A:E",
        ],
        "ooxml xlsx colwidths set" => &[
            "xlsx",
            "colwidths",
            "set",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
            "--sheet",
            "Sheet1",
            "--range",
            "A:E",
            "--width",
            "12",
            "--dry-run",
        ],
        "ooxml xlsx cells extract" => &[
            "xlsx",
            "cells",
            "extract",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
            "--sheet",
            "Sheet1",
            "--range",
            "A1",
        ],
        "ooxml xlsx cells clear" => &[
            "xlsx",
            "cells",
            "clear",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
            "--sheet",
            "Sheet1",
            "--ref",
            "A1",
            "--dry-run",
        ],
        "ooxml xlsx charts create" => &[
            "xlsx",
            "charts",
            "create",
            "testdata/xlsx/chart-workbook/workbook.xlsx",
            "--type",
            "bar",
            "--sheet",
            "Data",
            "--range",
            "B2:B4",
            "--categories",
            "A2:A4",
            "--dry-run",
        ],
        "ooxml xlsx freeze set" => &[
            "xlsx",
            "freeze",
            "set",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
            "--sheet",
            "Sheet1",
            "--rows",
            "2",
            "--cols",
            "1",
            "--dry-run",
        ],
        _ => panic!("missing successful alias fixture for {path}"),
    };
    std::iter::once("--json".to_string())
        .chain(args.iter().map(|arg| (*arg).to_string()))
        .collect()
}

#[test]
fn every_documented_flag_alias_reaches_its_leaf_parser() {
    let manifest = live_intent_manifest();
    assert_eq!(
        manifest.flag_aliases.len(),
        29,
        "explicit registry denominator changed; review and update this contract"
    );

    for record in &manifest.flag_aliases {
        let command = manifest
            .commands
            .iter()
            .find(|command| command.path == record.path)
            .unwrap_or_else(|| panic!("alias path absent from manifest: {}", record.path));
        for canonical in &record.canonical_flags {
            let flag = command
                .local_flags
                .iter()
                .find(|flag| flag.name == *canonical)
                .unwrap_or_else(|| {
                    panic!(
                        "{} maps {} to absent canonical flag {}",
                        record.path, record.alias, canonical
                    )
                });
            assert!(
                flag.aliases.contains(&record.alias),
                "{} {} missing from localFlags alias metadata for {}",
                record.path,
                record.alias,
                canonical
            );
        }

        let mut help_args = vec!["help".to_string()];
        help_args.extend(
            record
                .path
                .split_whitespace()
                .skip_while(|token| *token == "ooxml")
                .map(str::to_string),
        );
        let help_refs = help_args.iter().map(String::as_str).collect::<Vec<_>>();
        let help = run_ooxml_process(&help_refs);
        assert_eq!(help.code, 0, "help exit for {}", record.path);
        assert!(help.stderr.is_empty(), "help stderr for {}", record.path);
        let help = String::from_utf8(help.stdout).expect("help UTF-8");
        for canonical in &record.canonical_flags {
            let line = help
                .lines()
                .find(|line| line.trim_start().starts_with(canonical))
                .unwrap_or_else(|| panic!("help omitted {canonical} for {}", record.path));
            assert!(
                line.contains(&record.alias),
                "help omitted alias {} for {} {canonical}: {line}",
                record.alias,
                record.path
            );
        }

        let mut args = vec!["--json".to_string()];
        args.extend(
            record
                .path
                .split_whitespace()
                .skip_while(|token| *token == "ooxml")
                .map(str::to_string),
        );
        args.push(record.alias.clone());
        args.push(
            if record.path == "ooxml xlsx freeze set" {
                "B3"
            } else if matches!(
                record.alias.as_str(),
                "--after" | "--after-block" | "--insert-after" | "--block" | "--index"
            ) {
                "1"
            } else {
                "intent-value"
            }
            .to_string(),
        );
        let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
        let output = run_ooxml_process(&refs);
        assert_ne!(
            output.code, 0,
            "incomplete alias probe must reach the leaf parser: {args:?}"
        );
        assert!(
            output.stdout.is_empty() ^ output.stderr.is_empty(),
            "alias probe must emit on exactly one channel for {args:?}"
        );
        let bytes = if output.stdout.is_empty() {
            output.stderr
        } else {
            output.stdout
        };
        let text = String::from_utf8(bytes).expect("alias probe JSON is UTF-8");
        assert_eq!(text.lines().count(), 1, "one JSON object for {args:?}");
        let value: Value = serde_json::from_str(&text).expect("alias probe JSON object");
        let message = value["error"]["message"]
            .as_str()
            .expect("alias probe error message");
        assert!(
            !message.contains(&format!("unknown flag: {}", record.alias)),
            "registered alias was rejected before its leaf parser: {args:?}: {message}"
        );
    }

    assert_eq!(manifest.command_aliases.len(), 1);
    for record in &manifest.command_aliases {
        let command = manifest
            .commands
            .iter()
            .find(|command| command.path == record.canonical_command)
            .unwrap_or_else(|| {
                panic!(
                    "command alias {} maps to absent command {}",
                    record.alias, record.canonical_command
                )
            });
        assert!(
            command.aliases.contains(&record.alias),
            "{} omits command alias {}",
            record.canonical_command,
            record.alias
        );
        let help_args = record
            .alias
            .split_whitespace()
            .skip_while(|token| *token == "ooxml")
            .map(str::to_string)
            .collect::<Vec<_>>();
        let help_args = std::iter::once("help".to_string())
            .chain(help_args)
            .collect::<Vec<_>>();
        let refs = help_args.iter().map(String::as_str).collect::<Vec<_>>();
        let help = run_ooxml_process(&refs);
        assert_eq!(help.code, 0, "alias help: {}", record.alias);
        let help = String::from_utf8(help.stdout).expect("alias help UTF-8");
        assert!(help.contains(&record.alias), "{help}");
    }

    let robot_docs = explicit_json_success(
        &["--json", "robot-docs", "guide"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>(),
    );
    let documented = robot_docs["flagAliases"]
        .as_array()
        .expect("robot-docs flag alias table");
    assert_eq!(documented.len(), manifest.flag_aliases.len());
    for record in &manifest.flag_aliases {
        assert!(
            documented.iter().any(|row| {
                row["path"] == record.path
                    && row["alias"] == record.alias
                    && row["canonicalFlags"] == serde_json::json!(&record.canonical_flags)
            }),
            "robot-docs omitted {} {}",
            record.path,
            record.alias
        );
    }
    assert_eq!(
        robot_docs["commandAliases"],
        serde_json::to_value(&manifest.command_aliases)
            .expect("serialize command aliases for comparison")
    );
}

#[test]
fn every_registry_alias_matches_its_canonical_binary_envelope() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after epoch")
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-exhaustive-aliases-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("create exhaustive alias directory");
    let table_data = temp_dir.join("table.json");
    std::fs::write(&table_data, r#"[["Region","Amount"],["North",42]]"#).expect("write table data");
    let table_data = table_data.to_str().expect("table data path");
    let clip = temp_dir.join("clip.mp4");
    std::fs::write(&clip, b"opaque-fake-media-bytes").expect("write media clip");
    let clip = clip.to_str().expect("media path");
    let media_deck = temp_dir.join("media-deck.pptx");
    let media_deck = media_deck.to_str().expect("media deck path");
    let media = explicit_json_success(
        &[
            "--json",
            "pptx",
            "media",
            "add",
            "testdata/pptx/minimal-title/presentation.pptx",
            "--slide",
            "1",
            "--file",
            clip,
            "--name",
            "Alias Fixture",
            "--out",
            media_deck,
        ]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>(),
    );
    let media_shape = media["shapeId"]
        .as_u64()
        .expect("prepared media shape id")
        .to_string();
    let blocks = explicit_json_success(
        &[
            "--json",
            "docx",
            "blocks",
            "testdata/docx/styled-headings/document.docx",
            "--block",
            "1",
        ]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>(),
    );
    let block_hash = blocks["blocks"][0]["contentHash"]
        .as_str()
        .expect("block content hash");

    let manifest = live_intent_manifest();
    assert_eq!(manifest.flag_aliases.len(), 29, "review alias denominator");
    for record in &manifest.flag_aliases {
        let canonical_args = alias_base_invocation(
            &record.path,
            table_data,
            clip,
            media_deck,
            &media_shape,
            block_hash,
        );
        let alias_args = if record.canonical_flags.len() == 2 {
            let mut args = canonical_args.clone();
            let index = args
                .iter()
                .position(|arg| arg == &record.canonical_flags[0])
                .expect("freeze canonical --rows");
            assert_eq!(args.get(index + 2), record.canonical_flags.get(1));
            args.splice(index..index + 4, [record.alias.clone(), "B3".to_string()]);
            args
        } else {
            replace_flag(&canonical_args, &record.canonical_flags[0], &record.alias)
        };

        let mut canonical = explicit_json_success(&canonical_args);
        let mut alias = explicit_json_success(&alias_args);
        assert_eq!(
            alias["aliasesApplied"],
            serde_json::json!([{
                "alias": record.alias,
                "canonicalFlags": record.canonical_flags,
            }]),
            "{} {}",
            record.path,
            record.alias
        );
        remove_alias_readbacks(&mut canonical);
        remove_alias_readbacks(&mut alias);
        assert_eq!(
            alias, canonical,
            "{} {} diverged from {}",
            record.path, record.alias, record.canonical_flags[0]
        );
    }

    assert_eq!(manifest.command_aliases.len(), 1, "review command aliases");
    for record in &manifest.command_aliases {
        let canonical_args = alias_base_invocation(
            &record.canonical_command,
            table_data,
            clip,
            media_deck,
            &media_shape,
            block_hash,
        );
        let canonical_path = record
            .canonical_command
            .split_whitespace()
            .skip(1)
            .collect::<Vec<_>>();
        let alias_path = record.alias.split_whitespace().skip(1).collect::<Vec<_>>();
        let mut alias_args = vec!["--json".to_string()];
        alias_args.extend(alias_path.into_iter().map(str::to_string));
        alias_args.extend(canonical_args.into_iter().skip(1 + canonical_path.len()));

        let canonical_args = alias_base_invocation(
            &record.canonical_command,
            table_data,
            clip,
            media_deck,
            &media_shape,
            block_hash,
        );
        let mut canonical = explicit_json_success(&canonical_args);
        let mut alias = explicit_json_success(&alias_args);
        assert_eq!(
            alias["aliasesApplied"],
            serde_json::json!([{
                "alias": record.alias,
                "canonicalCommand": record.canonical_command,
            }])
        );
        remove_alias_readbacks(&mut canonical);
        remove_alias_readbacks(&mut alias);
        assert_eq!(alias, canonical, "command alias {}", record.alias);
    }

    std::fs::remove_dir_all(&temp_dir).expect("remove exhaustive alias directory");
}

#[test]
fn flag_aliases_execute_and_report_canonical_readback() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after epoch")
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-flag-aliases-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("create alias test directory");
    let table_data = temp_dir.join("table.json");
    std::fs::write(&table_data, r#"[["Region","Amount"],["North",42]]"#).expect("write table data");
    let table_data = table_data.to_str().expect("table data path");
    let clip = temp_dir.join("clip.mp4");
    std::fs::write(&clip, b"opaque-fake-media-bytes").expect("write media clip");
    let clip = clip.to_str().expect("media path");

    assert_alias_readback(
        &[
            "--json",
            "xlsx",
            "colwidths",
            "set",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
            "--sheet",
            "Sheet1",
            "--col",
            "A:E",
            "--width",
            "12",
            "--dry-run",
        ],
        serde_json::json!([{"alias": "--col", "canonicalFlags": ["--range"]}]),
    );
    assert_alias_readback(
        &[
            "--json",
            "xlsx",
            "freeze",
            "set",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
            "--sheet",
            "Sheet1",
            "--at",
            "B2",
            "--dry-run",
        ],
        serde_json::json!([{
            "alias": "--at",
            "canonicalFlags": ["--rows", "--cols"]
        }]),
    );
    assert_alias_readback(
        &[
            "--json",
            "docx",
            "styles",
            "apply",
            "testdata/docx/apply-styles/document.docx",
            "--block",
            "1",
            "--target",
            "paragraph",
            "--style",
            "Heading2",
            "--dry-run",
        ],
        serde_json::json!([{"alias": "--block", "canonicalFlags": ["--index"]}]),
    );
    assert_alias_readback(
        &[
            "--json",
            "docx",
            "paragraphs",
            "insert",
            "testdata/docx/styled-headings/document.docx",
            "--after",
            "0",
            "--text",
            "Alias paragraph",
            "--dry-run",
        ],
        serde_json::json!([{
            "alias": "--after",
            "canonicalFlags": ["--insert-after"]
        }]),
    );
    assert_alias_readback(
        &[
            "--json",
            "pptx",
            "place",
            "table",
            "testdata/pptx/minimal-title/presentation.pptx",
            "--slide",
            "1",
            "--values-file",
            table_data,
            "--data-format",
            "json",
            "--x",
            "0",
            "--y",
            "0",
            "--cx",
            "2000000",
            "--dry-run",
        ],
        serde_json::json!([
            {"alias": "--values-file", "canonicalFlags": ["--data"]},
            {"alias": "--data-format", "canonicalFlags": ["--format"]}
        ]),
    );
    assert_alias_readback(
        &[
            "--json",
            "pptx",
            "media",
            "add",
            "testdata/pptx/minimal-title/presentation.pptx",
            "--slide",
            "1",
            "--image",
            clip,
            "--dry-run",
        ],
        serde_json::json!([{"alias": "--image", "canonicalFlags": ["--file"]}]),
    );
    assert_alias_readback(
        &[
            "--json",
            "xlsx",
            "cells",
            "extract",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
            "--sheet",
            "Sheet1",
            "--cell",
            "A1",
        ],
        serde_json::json!([{"alias": "--cell", "canonicalFlags": ["--range"]}]),
    );
    assert_alias_readback(
        &[
            "--json",
            "docx",
            "blocks",
            "testdata/docx/styled-headings/document.docx",
            "--index",
            "1",
        ],
        serde_json::json!([{"alias": "--index", "canonicalFlags": ["--block"]}]),
    );

    let chart_output = temp_dir.join("two-range-chart.xlsx");
    let chart_output = chart_output.to_str().expect("chart output path");
    let chart = assert_alias_readback(
        &[
            "--json",
            "xlsx",
            "charts",
            "create",
            "testdata/xlsx/chart-workbook/workbook.xlsx",
            "--type",
            "bar",
            "--sheet",
            "Data",
            "--values",
            "B2:B4",
            "--categories",
            "A2:A4",
            "--out",
            chart_output,
        ],
        serde_json::json!([{"alias": "--values", "canonicalFlags": ["--range"]}]),
    );
    assert_eq!(chart["sourceRange"], "B2:B4");
    assert_eq!(chart["categoriesRange"], "A2:A4");
    assert_eq!(chart["seriesCount"], 1);
    assert_eq!(chart["categories"], 3);
    assert_intent_output_strict_valid(chart_output, "two-range chart alias output");
    let (_, shown, stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "charts",
        "show",
        chart_output,
        "--chart",
        "chart:2",
    ]);
    assert_eq!(stderr, None);
    let shown = shown.expect("two-range chart readback");
    assert_eq!(
        shown["charts"][0]["series"][0]["categories"]["formula"],
        "'Data'!$A$2:$A$4"
    );
    assert_eq!(
        shown["charts"][0]["series"][0]["values"]["formula"],
        "'Data'!$B$2:$B$4"
    );

    assert_alias_readback(
        &[
            "--json",
            "pptx",
            "slides",
            "add",
            "testdata/pptx/multi-layout/presentation.pptx",
            "--layout",
            "Title and Content",
            "--set-text",
            "title=Alias title",
            "--dry-run",
        ],
        serde_json::json!([{
            "alias": "ooxml pptx slides add",
            "canonicalCommand": "ooxml pptx new-slide-from-layout"
        }]),
    );

    std::fs::remove_dir_all(&temp_dir).expect("remove alias test directory");
}

#[test]
fn pptx_text_set_accepts_single_and_multi_paragraph_text() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after epoch")
        .as_nanos();
    for (label, expected) in [
        ("single", &["Solo paragraph"][..]),
        ("multi", &["Alpha", "Beta"][..]),
    ] {
        let output = std::env::temp_dir().join(format!(
            "ooxml-text-set-{label}-{}-{suffix}.pptx",
            std::process::id()
        ));
        let output = output.to_str().expect("text-set output path");
        let text = expected.join("\n");
        let value = explicit_json_success(
            &[
                "--json",
                "pptx",
                "text",
                "set",
                "testdata/pptx/multi-layout/presentation.pptx",
                "--slide",
                "2",
                "--target",
                "body",
                "--text",
                &text,
                "--out",
                output,
            ]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>(),
        );
        assert_eq!(value["mode"], "paragraph-content");
        assert_eq!(value["paragraphCount"], expected.len());
        assert_intent_output_strict_valid(output, "pptx text set --text output");

        let (_, shown, stderr) = run_ooxml(&[
            "--json",
            "pptx",
            "shapes",
            "get",
            output,
            "--slide",
            "2",
            "--target",
            "body",
            "--include-text",
        ]);
        assert_eq!(stderr, None);
        let paragraphs = shown.expect("text-set readback")["shapes"][0]["paragraphs"]
            .as_array()
            .cloned()
            .expect("paragraph readback array");
        assert_eq!(paragraphs.len(), expected.len());
        assert_eq!(
            paragraphs
                .iter()
                .map(|paragraph| paragraph["text"].as_str().expect("paragraph text"))
                .collect::<Vec<_>>(),
            expected
        );
        std::fs::remove_file(output).expect("remove text-set output");
    }
}

#[test]
fn xlsx_freeze_at_maps_cells_and_rejects_invalid_coordinates() {
    for (cell, rows, cols) in [("A2", 1, 0), ("B1", 0, 1), ("C3", 2, 2)] {
        let value = assert_alias_readback(
            &[
                "--json",
                "xlsx",
                "freeze",
                "set",
                "testdata/xlsx/minimal-workbook/workbook.xlsx",
                "--sheet",
                "Sheet1",
                "--at",
                cell,
                "--dry-run",
            ],
            serde_json::json!([{
                "alias": "--at",
                "canonicalFlags": ["--rows", "--cols"]
            }]),
        );
        assert_eq!(value["state"]["rows"], rows, "{cell}");
        assert_eq!(value["state"]["cols"], cols, "{cell}");
        assert_eq!(value["state"]["topLeftCell"], cell, "{cell}");
    }

    for cell in ["A0", "0A", "A2:B3"] {
        let output = run_ooxml_process(&[
            "--json",
            "xlsx",
            "freeze",
            "set",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
            "--sheet",
            "Sheet1",
            "--at",
            cell,
            "--dry-run",
        ]);
        assert_eq!(output.code, 2, "{cell}");
        assert!(output.stdout.is_empty(), "{cell}");
        let value: Value = serde_json::from_slice(&output.stderr).expect("JSON error on stderr");
        assert_eq!(value["error"]["code"], "invalid_args", "{cell}");
        assert!(
            value["error"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("invalid --at")),
            "{cell}: {value:?}"
        );
    }
}

#[test]
fn folded_manifest_rows_are_live_and_theme_derive_is_deterministic() {
    let manifest = live_intent_manifest();
    let command = |path: &str| {
        manifest
            .commands
            .iter()
            .find(|command| command.path == path)
            .unwrap_or_else(|| panic!("missing capability command {path}"))
    };
    let flags = |path: &str| {
        command(path)
            .local_flags
            .iter()
            .map(|flag| flag.name.as_str())
            .collect::<BTreeSet<_>>()
    };

    assert!(flags("ooxml render").is_superset(&BTreeSet::from([
        "--out", "--dpi", "--pages", "--sheet", "--format"
    ])));
    assert_eq!(flags("ooxml pptx theme derive"), BTreeSet::from(["--seed"]));
    assert!(flags("ooxml pptx scaffold").contains("--theme-seed"));
    assert!(flags("ooxml docx scaffold").is_superset(&BTreeSet::from([
        "--theme",
        "--theme-seed",
        "--template"
    ])));
    assert!(flags("ooxml pptx text set").is_superset(&BTreeSet::from([
        "--text",
        "--paragraphs-file",
        "--append"
    ])));
    assert!(flags("ooxml pptx add-textbox").contains("--paragraphs-file"));
    assert!(flags("ooxml pptx new-slide-from-layout").contains("--paragraphs-file"));
    for path in [
        "ooxml pptx add-textbox",
        "ooxml pptx place image",
        "ooxml pptx place table",
        "ooxml pptx place table-from-xlsx",
        "ooxml pptx charts create",
        "ooxml pptx media add",
    ] {
        assert!(
            flags(path).is_superset(&BTreeSet::from(["--slot", "--inset", "--aspect"])),
            "slot vocabulary missing from {path}"
        );
    }
    for path in [
        "ooxml docx paragraphs append",
        "ooxml docx paragraphs insert",
        "ooxml docx blocks replace",
        "ooxml docx blocks insert-after",
        "ooxml docx styles apply",
    ] {
        assert!(flags(path).contains("--create-style"), "{path}");
    }

    let first = explicit_json_success(
        &["--json", "pptx", "theme", "derive", "--seed", "1F4E79"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>(),
    );
    let second = explicit_json_success(
        &["--json", "pptx", "theme", "derive", "--seed", "#1f4e79"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>(),
    );
    assert_eq!(first, second);
    assert_eq!(first.as_object().expect("palette object").len(), 12);
    for key in [
        "dk1", "lt1", "dk2", "lt2", "accent1", "accent6", "hlink", "folHlink",
    ] {
        assert!(
            first[key].as_str().is_some_and(
                |color| color.len() == 6 && color.bytes().all(|byte| byte.is_ascii_hexdigit())
            ),
            "invalid palette color {key}: {}",
            first[key]
        );
    }
}

fn transpose_flag(flag: &str) -> Option<String> {
    let mut characters = flag.chars().collect::<Vec<_>>();
    for index in (2..characters.len().saturating_sub(1)).rev() {
        if characters[index] != characters[index + 1] {
            characters.swap(index, index + 1);
            return Some(characters.into_iter().collect());
        }
    }
    None
}

fn drop_flag_character(flag: &str) -> Option<String> {
    let mut characters = flag.chars().collect::<Vec<_>>();
    (characters.len() > 3).then(|| {
        characters.remove(characters.len() - 1);
        characters.into_iter().collect()
    })
}

fn wrong_flag_prefix(flag: &str) -> Option<String> {
    flag.strip_prefix("--").map(|name| format!("-{name}"))
}

fn toggle_flag_plural(flag: &str) -> Option<String> {
    let name = flag.strip_prefix("--")?;
    if let Some(singular) = name.strip_suffix('s') {
        (!singular.is_empty()).then(|| format!("--{singular}"))
    } else {
        Some(format!("--{name}s"))
    }
}

fn transpose_or_drop_token(token: &str) -> Option<String> {
    let mut characters = token.chars().collect::<Vec<_>>();
    for index in (0..characters.len().saturating_sub(1)).rev() {
        if characters[index] != characters[index + 1] {
            characters.swap(index, index + 1);
            return Some(characters.into_iter().collect());
        }
    }
    (characters.len() > 1).then(|| {
        characters.pop();
        characters.into_iter().collect()
    })
}

fn push_unique_case(
    cases: &mut Vec<GeneratedIntentCase>,
    seen_argv: &mut BTreeSet<Vec<String>>,
    case: GeneratedIntentCase,
) {
    if seen_argv.insert(case.argv.clone()) {
        cases.push(case);
    }
}

fn supports_preflight_flag_probes(command_path: &str, category: &str) -> bool {
    // These command surfaces reject malformed long flags before reading required
    // positional inputs. Keeping this list explicit prevents the corpus itself
    // from probing commands that can mutate a fixture before reporting the typo.
    if command_path == "ooxml capabilities" {
        return true;
    }
    category != "wrong-prefix"
        && matches!(
            command_path,
            "ooxml doctor"
                | "ooxml doctor health"
                | "ooxml conformance check"
                | "ooxml docx scaffold"
                | "ooxml pptx scaffold"
                | "ooxml xlsx scaffold"
                | "ooxml xlsx forms entry"
                | "ooxml vba build-bin"
                | "ooxml vba create"
                | "ooxml vba rebuild"
                | "ooxml vba run-smoke"
        )
}

fn manifest_derived_intent_cases(manifest: &IntentManifest) -> Vec<GeneratedIntentCase> {
    const GLOBAL_FLAGS: &[&str] = &["--format", "--json", "--strict"];
    let known_paths = manifest
        .commands
        .iter()
        .map(|command| command.path.clone())
        .collect::<BTreeSet<_>>();
    let mut cases = Vec::new();
    let mut seen_argv = BTreeSet::new();

    for command in &manifest.commands {
        let path_tokens = command
            .path
            .split_whitespace()
            .skip_while(|token| *token == "ooxml")
            .map(str::to_string)
            .collect::<Vec<_>>();
        let valid_flags = command
            .local_flags
            .iter()
            .map(|flag| flag.name.as_str())
            .chain(GLOBAL_FLAGS.iter().copied())
            .collect::<BTreeSet<_>>();

        for flag in &command.local_flags {
            let mutations = [
                ("transposed-flag", transpose_flag(&flag.name)),
                ("dropped-character", drop_flag_character(&flag.name)),
                ("wrong-prefix", wrong_flag_prefix(&flag.name)),
                ("plural-singular", toggle_flag_plural(&flag.name)),
            ];
            for (category, bad_flag) in mutations {
                if !supports_preflight_flag_probes(&command.path, category) {
                    continue;
                }
                let Some(bad_flag) = bad_flag else {
                    continue;
                };
                if bad_flag == flag.name || valid_flags.contains(bad_flag.as_str()) {
                    continue;
                }
                let mut argv = vec!["--json".to_string()];
                argv.extend(path_tokens.iter().cloned());
                argv.push(bad_flag.clone());
                if flag.flag_type != "bool" {
                    argv.push("intent-value".to_string());
                }
                push_unique_case(
                    &mut cases,
                    &mut seen_argv,
                    GeneratedIntentCase {
                        id: format!("{}:{}:{}", command.path, category, flag.name),
                        category: category.to_string(),
                        command_path: command.path.clone(),
                        argv,
                        bad_token: bad_flag,
                        canonical_token: flag.name.clone(),
                        expected_alias: false,
                    },
                );
            }
        }

        if let Some(canonical_root) = path_tokens.first()
            && let Some(wrong_root) = transpose_or_drop_token(canonical_root)
        {
            let mut wrong_path = path_tokens.clone();
            wrong_path[0] = wrong_root.clone();
            let mut argv = vec!["--json".to_string()];
            argv.extend(wrong_path);
            push_unique_case(
                &mut cases,
                &mut seen_argv,
                GeneratedIntentCase {
                    id: format!("{}:wrong-command-token:{canonical_root}", command.path),
                    category: "wrong-command-token".to_string(),
                    command_path: command.path.clone(),
                    argv,
                    bad_token: wrong_root,
                    canonical_token: canonical_root.clone(),
                    expected_alias: false,
                },
            );
        }

        let mut wrong_path = path_tokens.clone();
        let Some(canonical_verb) = wrong_path.last().cloned() else {
            continue;
        };
        let Some(wrong_verb) = transpose_or_drop_token(&canonical_verb) else {
            continue;
        };
        *wrong_path.last_mut().expect("nonempty command path") = wrong_verb.clone();
        let wrong_full_path = format!("ooxml {}", wrong_path.join(" "));
        let has_executable_prefix = (1..path_tokens.len()).any(|length| {
            known_paths.contains(&format!("ooxml {}", path_tokens[..length].join(" ")))
        });
        if !known_paths.contains(&wrong_full_path) && !has_executable_prefix {
            let mut argv = vec!["--json".to_string()];
            argv.extend(wrong_path);
            push_unique_case(
                &mut cases,
                &mut seen_argv,
                GeneratedIntentCase {
                    id: format!("{}:wrong-verb:{canonical_verb}", command.path),
                    category: "wrong-verb".to_string(),
                    command_path: command.path.clone(),
                    argv,
                    bad_token: wrong_verb,
                    canonical_token: canonical_verb,
                    expected_alias: false,
                },
            );
        }
    }

    for (id, argv, bad_token, canonical_token, expected_alias) in [
        (
            "sibling-range-col",
            &[
                "--json",
                "xlsx",
                "colwidths",
                "set",
                "testdata/xlsx/minimal-workbook/workbook.xlsx",
                "--sheet",
                "Sheet1",
                "--col",
                "A:E",
                "--width",
                "12",
                "--dry-run",
            ][..],
            "--col",
            "--range",
            true,
        ),
        (
            "sibling-freeze-at",
            &[
                "--json",
                "xlsx",
                "freeze",
                "set",
                "testdata/xlsx/minimal-workbook/workbook.xlsx",
                "--sheet",
                "Sheet1",
                "--at",
                "A2",
                "--dry-run",
            ][..],
            "--at",
            "--rows/--cols",
            true,
        ),
        (
            "sibling-chart-values",
            &[
                "--json",
                "xlsx",
                "charts",
                "create",
                "testdata/xlsx/chart-workbook/workbook.xlsx",
                "--type",
                "bar",
                "--sheet",
                "Data",
                "--values",
                "B2:B4",
                "--categories",
                "A2:A4",
                "--dry-run",
            ][..],
            "--values",
            "--range",
            true,
        ),
        (
            "sibling-docx-block",
            &[
                "--json",
                "docx",
                "styles",
                "apply",
                "testdata/docx/apply-styles/document.docx",
                "--block",
                "1",
                "--target",
                "paragraph",
                "--style",
                "Heading2",
                "--dry-run",
            ][..],
            "--block",
            "--index",
            true,
        ),
        (
            "sibling-pptx-text",
            &[
                "--json",
                "pptx",
                "text",
                "set",
                "testdata/pptx/multi-layout/presentation.pptx",
                "--slide",
                "2",
                "--target",
                "body",
                "--text",
                "X",
                "--dry-run",
            ][..],
            "--text",
            "--text",
            true,
        ),
        (
            "sibling-pptx-slides-add",
            &[
                "--json",
                "pptx",
                "slides",
                "add",
                "testdata/pptx/multi-layout/presentation.pptx",
                "--layout",
                "Title and Content",
                "--dry-run",
            ][..],
            "slides add",
            "new-slide-from-layout",
            true,
        ),
    ] {
        let argv: Vec<String> = argv.iter().map(|value| (*value).to_string()).collect();
        push_unique_case(
            &mut cases,
            &mut seen_argv,
            GeneratedIntentCase {
                id: id.to_string(),
                category: "sibling-concept".to_string(),
                command_path: argv
                    .iter()
                    .skip(1)
                    .take_while(|token| !token.starts_with('-'))
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(" "),
                argv,
                bad_token: bad_token.to_string(),
                canonical_token: canonical_token.to_string(),
                expected_alias,
            },
        );
    }

    cases
}

fn text_args_for(case: &GeneratedIntentCase) -> Vec<String> {
    let mut args = vec!["--format".to_string(), "text".to_string()];
    args.extend(
        case.argv
            .iter()
            .skip_while(|arg| arg.as_str() == "--json")
            .cloned(),
    );
    args
}

fn assert_text_projection(case: &GeneratedIntentCase, error: &Value, violations: &mut Vec<String>) {
    let args = text_args_for(case);
    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    let output = run_ooxml_process(&refs);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if output.code != 2 || !output.stdout.is_empty() {
        violations.push(format!(
            "{} text channel: exit {}, stdout {:?}",
            case.id,
            output.code,
            String::from_utf8_lossy(&output.stdout)
        ));
        return;
    }
    let Some(message) = error["message"].as_str() else {
        violations.push(format!("{} JSON error has no message", case.id));
        return;
    };
    for expected in [
        Some(format!("error [invalid_args]: {message}")),
        error["hint"].as_str().map(|value| format!("hint: {value}")),
        error["helpCommand"]
            .as_str()
            .map(|value| format!("help: {value}")),
    ]
    .into_iter()
    .flatten()
    {
        if !stderr.contains(&expected) {
            violations.push(format!(
                "{} text channel omitted {:?}: {stderr:?}",
                case.id, expected
            ));
        }
    }
    if let Some(suggestions) = error["didYouMean"].as_array()
        && !suggestions.is_empty()
    {
        let suggestions = suggestions
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join(", ");
        if !stderr.contains(&format!("did you mean: {suggestions}")) {
            violations.push(format!(
                "{} text channel omitted suggestions: {stderr:?}",
                case.id
            ));
        }
    }
    if error["correctedCommand"].is_string() && !stderr.contains("corrected command: ooxml ") {
        violations.push(format!(
            "{} text channel omitted corrected command: {stderr:?}",
            case.id
        ));
    }
}

#[test]
fn manifest_derived_wrong_invocations_are_never_silent_or_useless() {
    let manifest = live_intent_manifest();
    let cases = manifest_derived_intent_cases(&manifest);
    assert!(
        cases.len() >= 200,
        "manifest-derived corpus has only {} cases",
        cases.len()
    );
    assert_eq!(
        cases
            .iter()
            .filter(|case| case.category == "wrong-command-token")
            .count(),
        manifest.commands.len(),
        "every manifest leaf must contribute a wrong command-token invocation"
    );
    for category in [
        "transposed-flag",
        "dropped-character",
        "wrong-prefix",
        "plural-singular",
        "wrong-verb",
        "sibling-concept",
    ] {
        assert!(
            cases.iter().any(|case| case.category == category),
            "missing generated category {category}"
        );
    }

    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after epoch")
        .as_nanos();
    let log_dir = std::env::temp_dir().join(format!(
        "ooxml-intent-corpus-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&log_dir).expect("create intent corpus log directory");
    let log_path = log_dir.join("results.jsonl");
    let log_file = std::fs::File::create(&log_path).expect("create intent corpus JSONL");
    let mut log = BufWriter::new(log_file);
    let mut outcomes = BTreeMap::<IntentOutcome, usize>::new();
    let mut category_counts = BTreeMap::<String, usize>::new();
    let mut text_samples = BTreeMap::<String, usize>::new();
    let mut violations = Vec::new();

    for case in &cases {
        *category_counts.entry(case.category.clone()).or_default() += 1;
        let refs = case.argv.iter().map(String::as_str).collect::<Vec<_>>();
        let output = run_ooxml_process(&refs);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let parsed = serde_json::from_slice::<Value>(&output.stdout).ok();
        let error = parsed.as_ref().and_then(|value| value.get("error"));
        let error_code = error
            .and_then(|value| value["code"].as_str())
            .map(str::to_string);
        let message = error
            .and_then(|value| value["message"].as_str())
            .map(str::to_string);
        let did_you_mean = error
            .and_then(|value| value["didYouMean"].as_array())
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect::<Vec<_>>();
        let valid_flag_count = error
            .and_then(|value| value["validFlags"].as_array())
            .map_or(0, Vec::len);
        let token_rejected = message
            .as_deref()
            .is_some_and(|message| message.contains(&case.bad_token));
        let outcome = if output.code == 0 {
            if case.expected_alias {
                IntentOutcome::Inferred
            } else {
                IntentOutcome::SilentFail
            }
        } else if output.code == 2 && error_code.as_deref() == Some("invalid_args") {
            if case.expected_alias && !token_rejected {
                IntentOutcome::Inferred
            } else if token_rejected && (!did_you_mean.is_empty() || valid_flag_count > 0) {
                IntentOutcome::UsefulHint
            } else {
                IntentOutcome::UselessError
            }
        } else {
            IntentOutcome::UselessError
        };
        *outcomes.entry(outcome).or_default() += 1;

        if !case.expected_alias && output.code != 2 {
            violations.push(format!(
                "{} invalid invocation exited {} instead of 2",
                case.id, output.code
            ));
        }
        if !output.stderr.is_empty() {
            violations.push(format!(
                "{} explicit JSON contaminated stderr: {:?}",
                case.id,
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        if stdout.lines().count() != 1 || !parsed.as_ref().is_some_and(Value::is_object) {
            violations.push(format!(
                "{} did not emit exactly one JSON object: {stdout:?}",
                case.id
            ));
        }
        if matches!(
            outcome,
            IntentOutcome::SilentFail | IntentOutcome::UselessError
        ) {
            violations.push(format!(
                "{} classified {} (exit {}, message {:?})",
                case.id,
                outcome.label(),
                output.code,
                message
            ));
        }

        let observation = IntentObservation {
            case,
            outcome,
            exit_code: output.code,
            error_code,
            message,
            did_you_mean,
            valid_flag_count,
        };
        serde_json::to_writer(&mut log, &observation).expect("write intent corpus JSONL row");
        writeln!(log).expect("terminate intent corpus JSONL row");

        let samples = text_samples.entry(case.category.clone()).or_default();
        if case.category == "sibling-concept"
            && !case.expected_alias
            && *samples < 8
            && let Some(error) = error
        {
            assert_text_projection(case, error, &mut violations);
            *samples += 1;
        }
    }
    log.flush().expect("flush intent corpus JSONL");

    let expected_inferred = cases.iter().filter(|case| case.expected_alias).count();
    assert_eq!(
        expected_inferred, 6,
        "review the explicit first-guess intent denominator"
    );
    assert_eq!(
        outcomes
            .get(&IntentOutcome::Inferred)
            .copied()
            .unwrap_or_default(),
        expected_inferred,
        "all accepted first-guess forms must classify as inferred"
    );
    assert_eq!(
        outcomes
            .get(&IntentOutcome::UsefulHint)
            .copied()
            .unwrap_or_default(),
        cases.len() - expected_inferred,
        "all remaining wrong invocations must retain useful recovery hints"
    );

    eprintln!("intent corpus classification ({} cases)", cases.len());
    for outcome in [
        IntentOutcome::UsefulHint,
        IntentOutcome::Inferred,
        IntentOutcome::UselessError,
        IntentOutcome::SilentFail,
    ] {
        eprintln!(
            "  {:<14} {}",
            outcome.label(),
            outcomes.get(&outcome).copied().unwrap_or_default()
        );
    }
    eprintln!("intent corpus generation categories");
    for (category, count) in category_counts {
        eprintln!("  {category:<20} {count}");
    }
    eprintln!("intent corpus JSONL: {}", log_path.display());

    assert!(
        violations.is_empty(),
        "{} intent corpus violations; first failures:\n{}\nfull JSONL: {}",
        violations.len(),
        violations
            .iter()
            .take(30)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n"),
        log_path.display()
    );
    assert_eq!(
        outcomes
            .get(&IntentOutcome::UselessError)
            .copied()
            .unwrap_or_default(),
        0
    );
    assert_eq!(
        outcomes
            .get(&IntentOutcome::SilentFail)
            .copied()
            .unwrap_or_default(),
        0
    );
}

#[test]
fn corrected_command_reparses_and_dry_runs_a_manifest_mutation() {
    let input = "testdata/xlsx/minimal-workbook/workbook.xlsx";
    let wrong = [
        "--json",
        "xlsx",
        "colwidths",
        "set",
        input,
        "--sheet",
        "Sheet1",
        "--rnage",
        "A:E",
        "--width",
        "12",
        "--dry-run",
    ];
    let error = explicit_json_error(&wrong);
    let corrected = error["error"]["correctedCommand"]
        .as_str()
        .expect("corrected command");
    assert_eq!(
        corrected,
        format!(
            "ooxml --json xlsx colwidths set {input} --sheet Sheet1 --range A:E --width 12 --dry-run"
        )
    );
    let reparsed = corrected.split_whitespace().skip(1).collect::<Vec<_>>();
    let output = run_ooxml_process(&reparsed);
    assert_eq!(
        output.code,
        0,
        "corrected dry-run stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let value: Value = serde_json::from_slice(&output.stdout).expect("corrected dry-run JSON");
    assert_eq!(value["dryRun"], true);
    assert_eq!(value["range"], "A:E");
    assert_eq!(value["width"], 12);
}

#[test]
fn read_intent_corrections_export_ranges_without_mutating_the_workbook() {
    let input = "testdata/xlsx/minimal-workbook/workbook.xlsx";
    let before = std::fs::read(input).expect("input workbook");
    for (noun, verb) in [
        ("range", "get"),
        ("ranges", "get"),
        ("range", "read"),
        ("ranges", "show"),
        ("rnages", "export"),
    ] {
        let value = explicit_json_error(&[
            "--json", "xlsx", noun, verb, input, "--sheet", "Sheet1", "--range", "A1:B5",
        ]);
        let error = &value["error"];
        let expected =
            format!("ooxml --json xlsx ranges export {input} --sheet Sheet1 --range A1:B5");
        assert_eq!(
            error["correctedCommand"], expected,
            "{noun} {verb}: {error}"
        );
        assert!(
            error["didYouMean"]
                .as_array()
                .expect("suggestions")
                .iter()
                .all(|suggestion| suggestion != "ooxml xlsx ranges set"
                    && suggestion != "ooxml xlsx ranges replace")
        );
        let output = run_ooxml_process(&expected.split_whitespace().skip(1).collect::<Vec<_>>());
        assert_eq!(
            output.code,
            0,
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        let exported: Value = serde_json::from_slice(&output.stdout).expect("range readback");
        assert_eq!(exported["sheet"], "Sheet1");
        assert_eq!(exported["range"], "A1:B5");
        assert_eq!(std::fs::read(input).expect("unchanged input"), before);
    }
    let write = explicit_json_error(&["--json", "xlsx", "range", "set"]);
    assert_eq!(
        write["error"]["correctedCommand"],
        "ooxml --json xlsx ranges set"
    );
}

#[test]
fn invalid_args_text_mode_prints_the_same_recovery_fields_on_stderr() {
    let output = run_ooxml_process(&["--format", "text", "capabilities", "--fro", "xlsx"]);
    assert_eq!(output.code, 2);
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("text diagnostics are UTF-8");
    assert!(stderr.starts_with("error [invalid_args]: unknown flag: --fro"));
    assert!(stderr.contains("hint: did you mean --for?"));
    assert!(stderr.contains("did you mean: --for"));
    assert!(stderr.contains("valid flags:\n"));
    assert!(stderr.contains("help: ooxml help capabilities"));
    assert!(stderr.contains("corrected command: ooxml --format text capabilities --for xlsx"));
}

#[test]
fn invalid_args_suggestions_consult_the_shared_alias_registry() {
    let value = explicit_json_error(&[
        "--json",
        "xlsx",
        "colwidths",
        "set",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        "--sheet",
        "Sheet1",
        "--colsx",
        "A:E",
        "--width",
        "12",
        "--dry-run",
    ]);
    let error = &value["error"];
    assert!(
        error["didYouMean"]
            .as_array()
            .expect("flag suggestions")
            .iter()
            .any(|suggestion| suggestion == "--cols"),
        "registry alias missing from suggestions: {error:?}"
    );
    assert!(
        error["validFlags"]
            .as_array()
            .expect("valid flag inventory")
            .iter()
            .any(|flag| flag["flag"] == "--cols"),
        "registry alias missing from valid flags: {error:?}"
    );
}

#[test]
fn missing_required_flags_include_manifest_usage_and_valid_flag_inventory() {
    let value = explicit_json_error(&["--json", "xlsx", "colwidths", "set", "workbook.xlsx"]);
    let error = &value["error"];
    let hint = error["hint"].as_str().expect("required-argument hint");
    assert!(
        hint.contains("required flags: --sheet, --range, --width"),
        "{hint}"
    );
    assert!(
        hint.contains("Example: ooxml xlsx colwidths set <file> --sheet <sheet> --range <columns> --width <width>"),
        "{hint}"
    );
    assert_eq!(error["helpCommand"], "ooxml help xlsx colwidths set");
    assert!(error.get("didYouMean").is_none());
    assert!(error.get("correctedCommand").is_none());
    assert!(
        error["validFlags"]
            .as_array()
            .expect("valid flags")
            .iter()
            .any(|flag| flag == &serde_json::json!({"flag": "--range", "use": "--range <range>"}))
    );
}

#[test]
fn unrelated_invalid_args_keep_the_base_error_shape() {
    let output = run_ooxml_process(&[
        "inspect",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        "--format",
        "json",
    ]);
    assert_eq!(output.code, 2);
    assert!(output.stdout.is_empty());
    let value: Value = serde_json::from_slice(&output.stderr).expect("base JSON error");
    let error = value["error"].as_object().expect("error object");
    assert_eq!(error.len(), 3, "unrelated errors must remain additive-only");
    assert_eq!(error["code"], "invalid_args");
    assert_eq!(error["exitCode"], 2);
    assert_eq!(
        error["message"],
        "invalid command invocation 'inspect testdata/xlsx/minimal-workbook/workbook.xlsx --format json'; run `ooxml help` for usage or `ooxml --json capabilities` for the command inventory"
    );
}

#[test]
fn unknown_command_tokens_suggest_a_nearby_manifest_path() {
    let value = explicit_json_error(&["--json", "xlsx", "colwidhts", "set"]);
    let error = &value["error"];
    assert_eq!(
        error["message"],
        "unknown command token 'colwidhts'; run `ooxml help` for usage or `ooxml --json capabilities` for the command inventory"
    );
    assert!(
        error["didYouMean"]
            .as_array()
            .expect("command suggestions")
            .iter()
            .any(|command| command == "ooxml xlsx colwidths set"),
        "{error:?}"
    );
    assert_eq!(error["helpCommand"], "ooxml help");
    assert_eq!(error["correctedCommand"], "ooxml --json xlsx colwidths set");
}

#[test]
fn capabilities_golden_includes_the_documented_error_envelope() {
    let actual = run_ooxml_process(&["--json", "capabilities"]);
    assert_eq!(actual.code, 0);
    assert!(actual.stderr.is_empty());
    let golden_path = Path::new("testdata/golden/command-manifest-contract/capabilities.json");
    if std::env::var_os("UPDATE_GOLDENS").is_some() {
        std::fs::write(golden_path, &actual.stdout).expect("update reviewed capabilities golden");
    }
    let expected = std::fs::read(golden_path).expect("capabilities golden");
    assert_eq!(actual.stdout, expected, "capabilities golden drift");
    let document: Value = serde_json::from_slice(&actual.stdout).expect("capabilities JSON");
    assert_eq!(
        document["errorEnvelope"]["code"],
        "stable machine-readable error category"
    );
    assert_eq!(
        document["errorEnvelope"]["channels"]["explicitJson"],
        "one JSON object on stdout; diagnostics remain empty"
    );
}

#[test]
fn frozen_corpora_track_structured_errors_and_alias_help() {
    let help_path = "testdata/golden/command-manifest-contract/help-corpus.json";
    let mut help: FrozenHelpCorpus =
        serde_json::from_slice(&std::fs::read(help_path).expect("read frozen help corpus"))
            .expect("parse frozen help corpus");
    if std::env::var_os("UPDATE_GOLDENS").is_some() {
        let manifest = live_intent_manifest();
        sync_frozen_help_capability_cases(&mut help, &manifest);
    }
    for case in &mut help.cases {
        refresh_frozen_case(case);
    }

    let process_path = "testdata/golden/command-manifest-contract/process-matrix.json";
    let mut process: FrozenProcessMatrix =
        serde_json::from_slice(&std::fs::read(process_path).expect("read frozen process matrix"))
            .expect("parse frozen process matrix");
    for case in &mut process.cases {
        refresh_frozen_case(case);
    }

    if std::env::var_os("UPDATE_GOLDENS").is_some() {
        write_reviewed_golden(help_path, &help);
        write_reviewed_golden(process_path, &process);
    } else {
        let expected_help: FrozenHelpCorpus =
            serde_json::from_slice(&std::fs::read(help_path).expect("reread frozen help corpus"))
                .expect("reparse frozen help corpus");
        assert_eq!(
            help, expected_help,
            "frozen invalid-args drift in {help_path}"
        );

        let expected_process: FrozenProcessMatrix = serde_json::from_slice(
            &std::fs::read(process_path).expect("reread frozen process matrix"),
        )
        .expect("reparse frozen process matrix");
        assert_eq!(
            process, expected_process,
            "frozen invalid-args drift in {process_path}"
        );
    }
}
