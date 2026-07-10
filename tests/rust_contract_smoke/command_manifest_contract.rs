use super::*;
use serde::Deserialize;
use std::collections::BTreeSet;

const HELP_CORPUS_JSON: &str =
    include_str!("../../testdata/golden/command-manifest-contract/help-corpus.json");
const PROCESS_MATRIX_JSON: &str =
    include_str!("../../testdata/golden/command-manifest-contract/process-matrix.json");
const GROUP_TOPICS_JSON: &str =
    include_str!("../../testdata/golden/command-manifest-contract/group-topics.json");
const CAPABILITIES_JSON: &[u8] =
    include_bytes!("../../testdata/golden/command-manifest-contract/capabilities.json");

const INSPECT_PROMISE: &str = "call via inspect in serve/MCP";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HelpCorpus {
    schema_version: u32,
    group_count: usize,
    leaf_count: usize,
    group_topic_rows: usize,
    alias_owner_records: usize,
    alias_argv_count: usize,
    cases: Vec<ProcessCase>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProcessMatrix {
    schema_version: u32,
    cases: Vec<ProcessCase>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GroupTopicInventory {
    schema_version: u32,
    topics: Vec<GroupTopicInventoryEntry>,
}

#[derive(Deserialize)]
struct GroupTopicInventoryEntry {
    path: Vec<String>,
    aliases: Vec<String>,
}

#[derive(Deserialize)]
struct ProcessCase {
    name: String,
    category: String,
    argv: Vec<String>,
    status: i32,
    stdout: String,
    stderr: String,
}

#[test]
fn capabilities_match_exact_raw_bytes_and_frozen_contract_shapes() {
    let actual = run_ooxml_process(&["--json", "capabilities"]);
    assert_eq!(actual.code, 0, "capabilities exit");
    assert_eq!(actual.stderr, b"", "capabilities stderr");
    assert_eq!(actual.stdout, CAPABILITIES_JSON, "capabilities stdout");
    assert!(CAPABILITIES_JSON.ends_with(b"\n"));
    assert!(!CAPABILITIES_JSON.ends_with(b"\n\n"));
    assert!(!CAPABILITIES_JSON.contains(&b'\r'));

    let document: Value = serde_json::from_slice(CAPABILITIES_JSON).expect("capabilities JSON");
    let commands = document["commands"]
        .as_array()
        .expect("capability commands");
    assert_eq!(commands.len(), 309);
    let paths = commands
        .iter()
        .map(|command| {
            command["path"]
                .as_str()
                .expect("capability path")
                .to_string()
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        paths.len(),
        commands.len(),
        "capability paths must be unique"
    );
    assert_eq!(
        commands
            .iter()
            .filter(|command| command["opCompatible"] == true)
            .count(),
        70
    );

    let inspect_promises = commands
        .iter()
        .filter(|command| {
            command["opIneligibleReason"]
                .as_str()
                .is_some_and(|reason| reason.contains(INSPECT_PROMISE))
        })
        .collect::<Vec<_>>();
    assert_eq!(inspect_promises.len(), 23);
    assert!(
        inspect_promises
            .iter()
            .all(|command| command["opCompatible"] == false)
    );
    assert_eq!(
        inspect_promises
            .iter()
            .filter(|command| {
                command["opIneligibleReason"] == "read-only command; call via inspect in serve/MCP"
            })
            .count(),
        22
    );
    assert_eq!(
        command_by_path(commands, "ooxml docx tables show")["opIneligibleReason"],
        "read-only command; call via inspect in serve/MCP; generated table hashes feed hash-guarded DOCX table mutations"
    );

    let group = command_by_path(commands, "ooxml xlsx");
    assert_eq!(group["opCompatible"], false);
    assert_eq!(group["localFlags"], serde_json::json!([]));
    assert_eq!(group["targetObjectKinds"], serde_json::json!([]));
    assert_eq!(
        group["opIneligibleReason"],
        "it is a command group, not a leaf mutation command"
    );
    assert!(group.get("flagConstraints").is_none());

    let inspect = command_by_path(commands, "ooxml xlsx freeze show");
    assert_eq!(inspect["opCompatible"], false);
    assert_eq!(
        inspect["opIneligibleReason"],
        "read-only command; call via inspect in serve/MCP"
    );
    assert!(
        inspect["localFlags"]
            .as_array()
            .is_some_and(|flags| !flags.is_empty())
    );
    assert!(inspect.get("flagConstraints").is_none());

    let direct_only = command_by_path(commands, "ooxml pptx scaffold");
    assert_eq!(direct_only["opCompatible"], false);
    assert!(direct_only.get("opIneligibleReason").is_none());
    assert!(direct_only.get("flagConstraints").is_none());

    let mutation = command_by_path(commands, "ooxml xlsx cells set");
    assert_eq!(mutation["opCompatible"], true);
    assert!(mutation.get("opIneligibleReason").is_none());
    assert!(mutation.get("flagConstraints").is_none());
    assert!(
        mutation["localFlags"]
            .as_array()
            .is_some_and(|flags| !flags.is_empty())
    );

    let constrained = command_by_path(commands, "ooxml xlsx conditional-formats add");
    assert_eq!(constrained["opCompatible"], true);
    assert!(constrained.get("opIneligibleReason").is_none());
    assert!(constrained["flagConstraints"].is_object());
}

#[test]
fn exhaustive_help_discovery_corpus_matches_raw_process_bytes() {
    let corpus: HelpCorpus = serde_json::from_str(HELP_CORPUS_JSON).expect("help corpus JSON");
    assert_eq!(corpus.schema_version, 1);
    assert_eq!(corpus.group_count, 61);
    assert_eq!(corpus.leaf_count, 250);
    assert_eq!(corpus.group_topic_rows, 50);
    assert_eq!(corpus.alias_owner_records, 37);
    assert_eq!(corpus.alias_argv_count, 35);
    let inventory: GroupTopicInventory =
        serde_json::from_str(GROUP_TOPICS_JSON).expect("GROUP_TOPICS inventory JSON");
    assert_eq!(inventory.schema_version, 1);
    assert_eq!(inventory.topics.len(), corpus.group_topic_rows);
    assert_eq!(
        inventory
            .topics
            .iter()
            .map(|topic| topic.aliases.len())
            .sum::<usize>(),
        corpus.alias_owner_records
    );

    let categories = corpus
        .cases
        .iter()
        .map(|case| case.category.as_str())
        .collect::<Vec<_>>();
    assert_eq!(categories.iter().filter(|kind| **kind == "root").count(), 1);
    assert_eq!(
        categories
            .iter()
            .filter(|kind| **kind == "canonical-group")
            .count(),
        corpus.group_count
    );
    assert_eq!(
        categories
            .iter()
            .filter(|kind| **kind == "canonical-leaf")
            .count(),
        corpus.leaf_count
    );
    assert_eq!(
        categories.iter().filter(|kind| **kind == "alias").count(),
        corpus.alias_argv_count
    );
    for required in ["unknown", "ambiguous", "operation-ineligible"] {
        assert_eq!(
            categories.iter().filter(|kind| **kind == required).count(),
            1,
            "explicit help contract case {required}"
        );
    }
    let ambiguous = corpus
        .cases
        .iter()
        .find(|case| case.category == "ambiguous")
        .expect("ambiguous alias case");
    assert_eq!(ambiguous.status, 0);
    assert!(
        ambiguous
            .stdout
            .contains("Safe Rust-supported package conversion aliases."),
        "the colliding package alias must keep resolving to convert"
    );
    let help_package = corpus
        .cases
        .iter()
        .find(|case| case.category == "alias" && case.argv == ["help", "package"])
        .expect("deduplicated help package alias case");
    assert!(
        help_package
            .stdout
            .contains("Safe Rust-supported package conversion aliases."),
        "GROUP_TOPICS order must keep the package alias attached to convert"
    );
    let unknown = corpus
        .cases
        .iter()
        .find(|case| case.category == "unknown")
        .expect("unknown help case");
    assert_eq!(unknown.status, 2);
    assert_eq!(unknown.stdout, "");
    assert!(unknown.stderr.ends_with('\n'));
    let ineligible = corpus
        .cases
        .iter()
        .find(|case| case.category == "operation-ineligible")
        .expect("operation-ineligible help case");
    assert_eq!(ineligible.status, 0);
    assert_eq!(ineligible.stderr, "");

    let mut argv_seen = BTreeSet::new();
    for case in &corpus.cases {
        if case.category != "unknown" {
            assert_eq!(case.status, 0, "{} help status", case.name);
            assert_eq!(case.stderr, "", "{} help stderr", case.name);
        }
        assert!(
            argv_seen.insert(case.argv.clone()),
            "duplicate help corpus argv: {:?}",
            case.argv
        );
        assert_process_case(case);
    }
    let inventory_alias_argv = inventory
        .topics
        .iter()
        .flat_map(|topic| {
            topic.aliases.iter().filter_map(|alias| {
                let mut alias_path = topic.path[..topic.path.len() - 1].to_vec();
                alias_path.push(alias.clone());
                (alias_path != topic.path).then(|| {
                    std::iter::once("help".to_string())
                        .chain(alias_path)
                        .collect::<Vec<_>>()
                })
            })
        })
        .collect::<BTreeSet<_>>();
    let corpus_alias_argv = corpus
        .cases
        .iter()
        .filter(|case| case.category == "alias")
        .map(|case| case.argv.clone())
        .collect::<BTreeSet<_>>();
    assert_eq!(inventory_alias_argv.len(), corpus.alias_argv_count);
    assert_eq!(
        inventory_alias_argv, corpus_alias_argv,
        "committed GROUP_TOPICS aliases must exactly own the help alias corpus"
    );

    let (_, capabilities, stderr) = run_ooxml(&["--json", "capabilities"]);
    assert_eq!(stderr, None, "capabilities stderr");
    let capability_paths = capabilities.expect("capabilities stdout")["commands"]
        .as_array()
        .expect("capability commands")
        .iter()
        .map(|command| {
            command["path"]
                .as_str()
                .expect("capability path")
                .strip_prefix("ooxml ")
                .expect("ooxml capability prefix")
                .split_whitespace()
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .collect::<BTreeSet<_>>();
    let canonical_capability_topics = corpus
        .cases
        .iter()
        .filter(|case| {
            matches!(case.category.as_str(), "canonical-group" | "canonical-leaf")
                && case.argv.first().map(String::as_str) == Some("help")
        })
        .map(|case| case.argv[1..].to_vec())
        .filter(|topic| capability_paths.contains(topic))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        canonical_capability_topics, capability_paths,
        "help corpus must cover every capability path as a group or executable leaf"
    );
}

#[test]
fn completion_scripts_match_exact_raw_byte_goldens() {
    for (shell, expected) in [
        (
            "bash",
            include_bytes!("../../testdata/golden/command-manifest-contract/completion-bash.txt")
                .as_slice(),
        ),
        (
            "fish",
            include_bytes!("../../testdata/golden/command-manifest-contract/completion-fish.txt")
                .as_slice(),
        ),
        (
            "powershell",
            include_bytes!(
                "../../testdata/golden/command-manifest-contract/completion-powershell.txt"
            )
            .as_slice(),
        ),
        (
            "zsh",
            include_bytes!("../../testdata/golden/command-manifest-contract/completion-zsh.txt")
                .as_slice(),
        ),
    ] {
        let actual = run_ooxml_process(&["completion", shell]);
        assert_eq!(actual.code, 0, "completion {shell} exit");
        assert_eq!(actual.stderr, b"", "completion {shell} stderr");
        assert_eq!(actual.stdout, expected, "completion {shell} stdout");
        assert!(actual.stdout.ends_with(b"\n"), "completion {shell} newline");
        assert!(
            !actual.stdout.ends_with(b"\n\n"),
            "completion {shell} must end in exactly one LF"
        );
        assert!(
            !actual.stdout.contains(&b'\r'),
            "completion {shell} must contain LF rather than CRLF"
        );
    }
}

#[test]
fn raw_first_global_flag_and_output_contract_matrix_matches() {
    let matrix: ProcessMatrix =
        serde_json::from_str(PROCESS_MATRIX_JSON).expect("process matrix JSON");
    assert_eq!(matrix.schema_version, 1);
    assert_eq!(matrix.cases.len(), 18);
    let mut names = BTreeSet::new();
    for case in &matrix.cases {
        assert!(names.insert(case.name.as_str()), "duplicate matrix name");
        assert_process_case(case);
    }

    for name in ["serve-eof", "serve-json-eof", "mcp-eof", "mcp-json-eof"] {
        let case = matrix
            .cases
            .iter()
            .find(|case| case.name == name)
            .expect("raw-first EOF case");
        assert_eq!(case.status, 0);
        assert_eq!(case.stdout, "");
        assert_eq!(case.stderr, "");
    }
    for name in ["json-leading", "json-trailing"] {
        let case = matrix
            .cases
            .iter()
            .find(|case| case.name == name)
            .expect("JSON success case");
        assert!(case.stdout.ends_with('\n'), "{name} stdout newline");
        assert_eq!(case.stderr, "");
    }
    let text = matrix
        .cases
        .iter()
        .find(|case| case.name == "text-success-newline")
        .expect("text success case");
    assert!(text.stdout.ends_with('\n'));
    assert_eq!(text.stderr, "");
    for name in [
        "json-before-serve",
        "json-before-mcp",
        "json-error-newline",
        "json-typo-error",
        "format-text-error",
    ] {
        let case = matrix
            .cases
            .iter()
            .find(|case| case.name == name)
            .expect("JSON error case");
        assert_eq!(case.stdout, "", "{name} stdout");
        assert!(case.stderr.ends_with('\n'), "{name} stderr newline");
    }
}

fn assert_process_case(case: &ProcessCase) {
    let args = case.argv.iter().map(String::as_str).collect::<Vec<_>>();
    let actual = run_ooxml_process(&args);
    assert_eq!(
        actual.code, case.status,
        "{} exit for {:?}",
        case.name, args
    );
    assert_eq!(
        actual.stdout,
        case.stdout.as_bytes(),
        "{} stdout for {:?}",
        case.name,
        args
    );
    assert_eq!(
        actual.stderr,
        case.stderr.as_bytes(),
        "{} stderr for {:?}",
        case.name,
        args
    );
}

fn command_by_path<'a>(commands: &'a [Value], path: &str) -> &'a Value {
    commands
        .iter()
        .find(|command| command["path"].as_str() == Some(path))
        .expect("missing representative capability command")
}
