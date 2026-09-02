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
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IntentCommand {
    path: String,
    #[serde(default)]
    local_flags: Vec<IntentFlag>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IntentFlag {
    name: String,
    #[serde(rename = "type")]
    flag_type: String,
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

    for (id, argv, bad_token, canonical_token) in [
        (
            "sibling-range-col",
            &["--json", "xlsx", "colwidths", "set", "--col", "A:E"][..],
            "--col",
            "--range",
        ),
        (
            "sibling-freeze-cell",
            &["--json", "xlsx", "freeze", "set", "--cell", "A2"][..],
            "--cell",
            "--at",
        ),
        (
            "sibling-chart-values",
            &["--json", "xlsx", "charts", "create", "--values", "[]"][..],
            "--values",
            "--range",
        ),
        (
            "sibling-docx-block",
            &["--json", "docx", "styles", "apply", "--block", "1"][..],
            "--block",
            "--index",
        ),
        (
            "sibling-pptx-text",
            &["--json", "pptx", "text", "set", "--text", "X"][..],
            "--text",
            "pptx replace text",
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
                expected_alias: false,
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
            && *samples < 8
            && let Some(error) = error
        {
            assert_text_projection(case, error, &mut violations);
            *samples += 1;
        }
    }
    log.flush().expect("flush intent corpus JSONL");

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
        "--col",
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
fn invalid_args_envelope_redirects_the_five_known_first_guess_failures() {
    let cases = [
        (
            vec!["--json", "xlsx", "colwidths", "set", "--col", "A:E"],
            serde_json::json!(["--range"]),
            Some("ooxml --json xlsx colwidths set --range A:E"),
        ),
        (
            vec!["--json", "xlsx", "freeze", "set", "--cell", "A2"],
            serde_json::json!(["--rows", "--cols"]),
            None,
        ),
        (
            vec!["--json", "xlsx", "charts", "create", "--values", "[]"],
            serde_json::json!(["--range", "--table"]),
            None,
        ),
        (
            vec!["--json", "docx", "styles", "apply", "--block", "1"],
            serde_json::json!(["--index"]),
            Some("ooxml --json docx styles apply --index 1"),
        ),
        (
            vec!["--json", "pptx", "text", "set", "--text", "X"],
            serde_json::json!(["ooxml pptx replace text"]),
            None,
        ),
    ];

    for (args, suggestions, corrected) in cases {
        let value = explicit_json_error(&args);
        let error = &value["error"];
        assert_eq!(error["code"], "invalid_args", "{args:?}");
        assert_eq!(error["exitCode"], 2, "{args:?}");
        assert_eq!(error["didYouMean"], suggestions, "{args:?}");
        assert!(
            error["validFlags"]
                .as_array()
                .is_some_and(|flags| !flags.is_empty()),
            "{args:?}: {error:?}"
        );
        assert!(
            error["helpCommand"]
                .as_str()
                .is_some_and(|command| command.starts_with("ooxml help ")),
            "{args:?}: {error:?}"
        );
        assert!(
            error["hint"].as_str().is_some_and(|hint| !hint.is_empty()),
            "{args:?}: {error:?}"
        );
        assert_eq!(error["correctedCommand"].as_str(), corrected, "{args:?}");
    }
}

#[test]
fn invalid_args_text_mode_prints_the_same_recovery_fields_on_stderr() {
    let output = run_ooxml_process(&[
        "--format",
        "text",
        "xlsx",
        "colwidths",
        "set",
        "--col",
        "A:E",
    ]);
    assert_eq!(output.code, 2);
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("text diagnostics are UTF-8");
    assert!(stderr.starts_with("error [invalid_args]: unknown flag: --col\n"));
    assert!(stderr.contains("hint: column spans use --range"));
    assert!(stderr.contains("did you mean: --range"));
    assert!(stderr.contains("valid flags:\n"));
    assert!(stderr.contains("help: ooxml help xlsx colwidths set"));
    assert!(
        stderr.contains("corrected command: ooxml --format text xlsx colwidths set --range A:E")
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
fn invalid_args_frozen_corpora_track_the_structured_error_contract() {
    let help_path = "testdata/golden/command-manifest-contract/help-corpus.json";
    let mut help: FrozenHelpCorpus =
        serde_json::from_slice(&std::fs::read(help_path).expect("read frozen help corpus"))
            .expect("parse frozen help corpus");
    for case in help.cases.iter_mut().filter(|case| case.status != 0) {
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
