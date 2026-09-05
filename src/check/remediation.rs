//! Remediation uses generated command text as data, never as shell input.
use crate::{CliError, CliResult};
use serde_json::{Map, Value, json};
use std::collections::BTreeSet;
use std::fs;

pub(crate) struct Options {
    pub(crate) out: Option<String>,
    pub(crate) in_place: bool,
    pub(crate) backup: Option<String>,
    pub(crate) dry_run: bool,
    pub(crate) max_rounds: usize,
}

pub(crate) fn parse_options(
    file: &str,
    args: &[String],
) -> CliResult<(Option<Options>, Vec<String>)> {
    let fix = crate::has_flag(args, "--fix");
    let mut read_args = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        if [
            "--fix",
            "--dry-run",
            "--in-place",
            "--out",
            "--backup",
            "--max-rounds",
        ]
        .contains(&arg.as_str())
        {
            if !fix {
                return Err(CliError::invalid_args(format!("{arg} requires --fix")));
            }
            index += if ["--out", "--backup", "--max-rounds"].contains(&arg.as_str()) {
                2
            } else {
                1
            };
        } else {
            read_args.push(arg.clone());
            index += 1;
        }
    }
    if !fix {
        return Ok((None, read_args));
    }
    let dry_run = crate::has_flag(args, "--dry-run");
    let in_place = crate::has_flag(args, "--in-place");
    let out = crate::parse_string_flag(args, "--out")?.or_else(|| {
        (!dry_run && !in_place).then(|| crate::design_check::fixed_output_path(file, "fixed"))
    });
    let max_rounds = crate::parse_string_flag(args, "--max-rounds")?
        .map(|value| {
            value
                .parse::<usize>()
                .map_err(|_| CliError::invalid_args("--max-rounds must be between 1 and 100"))
        })
        .transpose()?
        .unwrap_or(8);
    Ok((
        Some(Options {
            out,
            in_place,
            backup: crate::parse_string_flag(args, "--backup")?,
            dry_run,
            max_rounds,
        }),
        read_args,
    ))
}

struct Stage(String);
impl Drop for Stage {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

/// Each round uses the apply engine, but all rounds remain private until the
/// final strict assertion and readback have succeeded.
pub(crate) fn remediate(
    file: &str,
    command: &str,
    options: Options,
    inspect: impl Fn(&str) -> CliResult<Value>,
) -> CliResult<Value> {
    crate::validate_xlsx_mutation_output_flags(
        options.out.as_deref(),
        options.in_place,
        options.backup.as_deref(),
        options.dry_run,
    )?;
    if !(1..=100).contains(&options.max_rounds) {
        return Err(CliError::invalid_args(
            "--max-rounds must be between 1 and 100",
        ));
    }
    let stage = Stage(crate::mutation_staging_path(
        file,
        options.out.as_deref(),
        "remediate",
    ));
    fs::copy(file, &stage.0)
        .map_err(|error| CliError::unexpected(format!("cannot stage remediation: {error}")))?;
    let ops_file = Stage(format!("{}.ops.json", stage.0));
    let mut report = inspect(&stage.0)?;
    let before = display_paths(report.clone(), &stage.0, file);
    let mut rounds = Vec::new();
    let mut seen = BTreeSet::new();
    let termination;
    loop {
        let findings = report["findings"].as_array().cloned().unwrap_or_default();
        if findings.is_empty() {
            termination = "clean";
            break;
        }
        let mut ops = Vec::new();
        let mut items = Vec::new();
        for finding in findings {
            match operation(finding["fixCommand"].as_str().unwrap_or_default(), &stage.0) {
                Ok(mut op) => {
                    // The style repair also has to work when Normal itself is
                    // absent, as it can be in an imported minimal document.
                    if finding["code"] == "DOCX_DANGLING_STYLE" && op["command"] == "docx styles apply" {
                        op["args"]["create-style"] = json!(true);
                    }
                    if !ops.contains(&op) { ops.push(op.clone()); }
                    items.push(json!({"finding":finding,"before":"present","op":op}));
                }
                Err(reason) => items.push(json!({"finding":finding,"before":"present","after":"unresolved","op":null,"reason":reason})),
            }
        }
        if ops.is_empty() {
            termination = "no-fix";
            break;
        }
        if rounds.len() == options.max_rounds {
            termination = "max-rounds";
            break;
        }
        let fingerprint = serde_json::to_string(&ops).expect("serialize remediation ops");
        if !seen.insert(fingerprint) {
            termination = "no-progress";
            break;
        }
        fs::write(
            &ops_file.0,
            serde_json::to_vec(&ops).expect("serialize ops"),
        )
        .map_err(|error| CliError::unexpected(format!("cannot stage remediation plan: {error}")))?;
        let applied = crate::apply(
            &stage.0,
            &[
                "--ops".to_string(),
                ops_file.0.clone(),
                "--in-place".to_string(),
            ],
        )?;
        report = inspect(&stage.0)?;
        let after = report["findings"].as_array().cloned().unwrap_or_default();
        for item in &mut items {
            if !item["op"].is_null() {
                item["after"] = json!(if after
                    .iter()
                    .any(|finding| same_finding(finding, &item["finding"]))
                {
                    "remaining"
                } else {
                    "resolved"
                });
                let index = ops.iter().position(|op| op == &item["op"]).unwrap();
                item["readback"] = applied["applied"][index]["readback"].clone();
            }
        }
        rounds.push(json!({"round":rounds.len()+1,"findings":items,"ops":ops}));
    }
    let validation = crate::validation::validate(&stage.0, true)?;
    let validated = crate::validation::validate_exit_code(&validation, true) == crate::EXIT_SUCCESS;
    let committed = validated && !options.dry_run;
    let output = if options.in_place {
        file
    } else {
        options.out.as_deref().unwrap_or(file)
    };
    let unresolved = report["findings"].clone();
    let mut result = json!({
        "file":file, "output":if committed { json!(output) } else { Value::Null },
        "dryRun":options.dry_run, "committed":committed, "validated":validated,
        "remediation": {"before":before,"after":display_paths(report.clone(), &stage.0, output),"rounds":display_paths(json!(rounds), &stage.0, file),"roundsRun":rounds.len(),"maxRounds":options.max_rounds,"termination":termination,"unresolved":display_paths(unresolved, &stage.0, output)},
    });
    let mut envelope = json!({"output":output,"validated":validated});
    crate::mutation_envelope::attach_cli_mutation_envelope(
        &[
            "apply".to_string(),
            file.to_string(),
            "--out".to_string(),
            output.to_string(),
        ],
        Vec::new(),
        &mut envelope,
    )?;
    result["mutationEnvelope"] = envelope["mutationEnvelope"].clone();
    result["mutationEnvelope"]["command"] = json!(command);
    if rounds.is_empty() {
        result["mutationEnvelope"]["changed"] = json!([]);
    }
    if !validated {
        result["validation"] = display_paths(validation, &stage.0, file);
    }
    crate::finish_mutation_output(
        file,
        &stage.0,
        options.out.as_deref(),
        options.in_place,
        options.backup.as_deref(),
        !committed,
    )?;
    Ok(result)
}

fn same_finding(left: &Value, right: &Value) -> bool {
    left["code"] == right["code"] && left["location"] == right["location"]
}

fn display_paths(value: Value, stage: &str, file: &str) -> Value {
    match value {
        Value::String(value) => {
            let rewrite = |value: &str| {
                if value == stage {
                    return file.to_string();
                }
                let stage_stem = stage.rsplit_once('.').map_or(stage, |(stem, _)| stem);
                let file_stem = file.rsplit_once('.').map_or(file, |(stem, _)| stem);
                if let Some(suffix) = value.strip_prefix(&format!("{stage_stem}.")) {
                    return format!("{file_stem}.{suffix}");
                }
                value
                    .replace(&crate::command_arg(stage), &crate::command_arg(file))
                    .replace(stage, file)
            };
            if value.starts_with("ooxml ")
                && let Ok(words) = command_words(&value)
            {
                Value::String(
                    words
                        .iter()
                        .map(|word| crate::command_arg(&rewrite(word)))
                        .collect::<Vec<_>>()
                        .join(" "),
                )
            } else {
                Value::String(rewrite(&value))
            }
        }
        Value::Array(values) => Value::Array(
            values
                .into_iter()
                .map(|value| display_paths(value, stage, file))
                .collect(),
        ),
        Value::Object(values) => Value::Object(
            values
                .into_iter()
                .map(|(key, value)| (key, display_paths(value, stage, file)))
                .collect(),
        ),
        value => value,
    }
}

/// Convert only published, batch-compatible mutations. Diagnostic commands and
/// incomplete suggestions remain findings for the caller to explain unchanged.
fn operation(command: &str, file: &str) -> Result<Value, String> {
    let words = command_words(command)?;
    let words = words
        .strip_prefix(&["ooxml".to_string()])
        .ok_or("not an ooxml command")?;
    let words = words.strip_prefix(&["--json".to_string()]).unwrap_or(words);
    let capabilities = crate::capabilities::capability_commands();
    let capability = capabilities
        .iter()
        .filter(|row| row["opCompatible"] == true)
        .find(|row| {
            let path = row["path"]
                .as_str()
                .unwrap_or_default()
                .trim_start_matches("ooxml ")
                .split_whitespace()
                .collect::<Vec<_>>();
            words.len() > path.len()
                && words
                    .iter()
                    .zip(&path)
                    .all(|(word, expected)| word == expected)
                && words[path.len()] == file
        })
        .ok_or("fixCommand is not a batch-compatible mutation for this package")?;
    let path = capability["path"]
        .as_str()
        .unwrap()
        .trim_start_matches("ooxml ");
    let mut rest = &words[path.split_whitespace().count() + 1..];
    let mut args = Map::new();
    while let Some(word) = rest.first() {
        let flag = capability["localFlags"]
            .as_array()
            .unwrap()
            .iter()
            .find(|flag| flag["name"].as_str() == Some(word))
            .ok_or_else(|| format!("unrecognized fix argument {word:?}"))?;
        let boolean = flag["type"] == "bool";
        let value = if boolean {
            json!(true)
        } else {
            json!(
                rest.get(1)
                    .ok_or_else(|| format!("missing value for {word}"))?
            )
        };
        rest = &rest[if boolean { 1 } else { 2 }..];
        if matches!(
            word.as_str(),
            "--out" | "--in-place" | "--backup" | "--dry-run"
        ) {
            continue;
        }
        if word == "--no-validate" {
            return Err("fixCommand cannot bypass strict validation".to_string());
        }
        let key = word.trim_start_matches("--");
        if args.insert(key.to_string(), value).is_some() {
            return Err(format!("duplicate fix argument {word}"));
        }
    }
    Ok(json!({"command": path, "args": args}))
}

/// Inverse of command_arg's single-quote encoding. No expansion, escaping,
/// pipelines, subprocesses, or platform shell interpretation occurs here.
fn command_words(command: &str) -> Result<Vec<String>, String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quote = None;
    let mut started = false;
    for ch in command.chars() {
        if quote == Some(ch) {
            quote = None;
        } else if quote.is_some() {
            word.push(ch);
        } else if matches!(ch, '\'' | '"') {
            quote = Some(ch);
            started = true;
        } else if ch.is_whitespace() {
            if started {
                words.push(std::mem::take(&mut word));
                started = false;
            }
        } else {
            word.push(ch);
            started = true;
        }
    }
    if quote.is_some() {
        return Err("unterminated quote in fixCommand".to_string());
    }
    if started {
        words.push(word);
    }
    Ok(words)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(file: &str) -> CliResult<Value> {
        crate::check::inspect(file, &json!({"openxmlSdk":"skip"}))
    }

    #[test]
    fn remediation_repairs_seeded_family_defects_and_dry_run_preserves_destination() {
        for (source, code) in [
            (
                "testdata/docx/scaffold-styles/dangling-style.docx",
                "DOCX_DANGLING_STYLE",
            ),
            (
                "testdata/invalid/missing-chart-source.xlsx",
                "XLSX_CHART_SOURCE_INVALID",
            ),
            (
                "testdata/pptx/layout-qa/inherited-title-chart-overlap/presentation.pptx",
                "PPTX_SHAPE_COLLISION",
            ),
        ] {
            let out = Stage(crate::package_mutation_temp_path(
                source,
                "remediation-test",
            ));
            let original = fs::read(source).unwrap();
            fs::write(&out.0, b"existing output").unwrap();
            for dry_run in [true, false] {
                let report = remediate(
                    source,
                    "check",
                    Options {
                        out: (!dry_run).then(|| out.0.clone()),
                        in_place: false,
                        backup: None,
                        dry_run,
                        max_rounds: 8,
                    },
                    check,
                )
                .unwrap();
                assert_eq!(report["validated"], true);
                assert!(report["remediation"]["roundsRun"].as_u64().unwrap() > 0);
                assert!(
                    !report["remediation"]["unresolved"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|finding| finding["code"] == code),
                    "{report}"
                );
                assert!(report["mutationEnvelope"].is_object(), "{report}");
                assert_eq!(fs::read(source).unwrap(), original);
                if dry_run {
                    assert_eq!(fs::read(&out.0).unwrap(), b"existing output");
                } else {
                    crate::validate_mutation_output(&out.0).unwrap();
                }
            }
        }
    }

    #[test]
    fn remediation_stops_repeated_plans_and_preserves_unfixable_teaching_text() {
        let source = "testdata/xlsx/minimal-workbook/workbook.xlsx";
        let report = remediate(source, "check", Options {out:None,in_place:false,backup:None,dry_run:true,max_rounds:8}, |file| {
            Ok(json!({"findings":[{"code":"NEEDS_REVIEW","message":"Keep this teaching text.","fixCommand":""},{"code":"UNCHANGED","fixCommand":format!("ooxml --json xlsx cells set {} --sheet Sheet1 --cell A1 --value 1 --out ignored.xlsx",crate::command_arg(file))}]}))
        }).unwrap();
        assert_eq!(report["remediation"]["termination"], "no-progress");
        assert_eq!(report["remediation"]["roundsRun"], 1);
        assert_eq!(
            report["remediation"]["unresolved"][0]["message"],
            "Keep this teaching text."
        );
    }

    #[test]
    fn generated_quoting_round_trips_native_paths_and_untrusted_text() {
        for value in [
            "",
            "plain",
            "C:\\Program Files\\it's.xlsx",
            "a\nb\tc",
            "$(touch /tmp/no); `cmd` | & < >",
        ] {
            assert_eq!(command_words(&crate::command_arg(value)).unwrap(), [value]);
        }
        assert!(command_words("'unfinished").is_err());
    }

    #[test]
    fn converts_finding_fixes_without_destination_flags() {
        let file = "C:\\Office Files\\it's.docx";
        let op = operation(&format!("ooxml --json docx styles apply {} --target paragraph --index 1 --style Normal --create-style --out elsewhere.docx", crate::command_arg(file)), file).unwrap();
        assert_eq!(
            op,
            json!({"command":"docx styles apply", "args":{"target":"paragraph","index":"1","style":"Normal","create-style":true}})
        );
        assert!(operation("ooxml --json outline a.docx", "a.docx").is_err());
        assert!(
            operation(
                "ooxml --json docx styles apply other.docx --style Normal",
                "a.docx"
            )
            .is_err()
        );
        assert!(
            operation(
                "ooxml --json docx styles apply a.docx --no-validate",
                "a.docx"
            )
            .is_err()
        );
    }
}
