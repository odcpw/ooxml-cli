use crate::agent_aliases::invalid_args_intent_hint;
use crate::cli_core::{EnrichedCliError, InvalidArgsDetails, InvalidArgsFlag};
use crate::{CliError, CliResult, command_arg};

const GLOBAL_ERROR_FLAGS: &[(&str, &str)] = &[
    ("--format", "--format <json|text>"),
    ("--json", "--json"),
    ("--strict", "--strict"),
];

pub(crate) fn enrich_invalid_args(raw_args: &[String], err: CliError) -> EnrichedCliError {
    if err.code != "invalid_args" {
        return EnrichedCliError {
            error: err,
            details: None,
        };
    }
    let projection = crate::command_manifest::error_projection_for_argv(raw_args);
    let unknown_flag = unknown_flag_from_message(&err.message);
    let unknown_command = err.message.starts_with("unknown command token");
    let missing_required_flags = projection.as_ref().is_some_and(|command| {
        missing_required_arguments(&err.message)
            && command.required_flags().iter().any(|flag| {
                !std::iter::once(*flag)
                    .chain(crate::agent_aliases::flag_aliases_for(&command.path, flag))
                    .any(|accepted| argv_has_flag(raw_args, accepted))
            })
    });
    if unknown_flag.is_none() && !unknown_command && !missing_required_flags {
        return EnrichedCliError {
            error: err,
            details: None,
        };
    }

    let mut valid_flags = projection
        .as_ref()
        .map(|command| {
            command
                .local_flags
                .iter()
                .flat_map(|flag| {
                    std::iter::once(flag.name)
                        .chain(crate::agent_aliases::flag_aliases_for(
                            &command.path,
                            flag.name,
                        ))
                        .map(|name| InvalidArgsFlag {
                            flag: name.to_string(),
                            use_text: if flag.flag_type == "bool" {
                                name.to_string()
                            } else {
                                format!("{} <{}>", name, flag.arg_name)
                            },
                        })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    valid_flags.extend(
        GLOBAL_ERROR_FLAGS
            .iter()
            .map(|(flag, use_text)| InvalidArgsFlag {
                flag: (*flag).to_string(),
                use_text: (*use_text).to_string(),
            }),
    );
    valid_flags.sort_by(|left, right| left.flag.cmp(&right.flag));
    valid_flags.dedup_by(|left, right| left.flag == right.flag);

    let mut did_you_mean = Vec::new();
    let mut hint = None;
    if let Some(wrong_flag) = unknown_flag.as_deref() {
        if let Some(command) = projection.as_ref()
            && let Some(intent) = invalid_args_intent_hint(&command.path, wrong_flag)
        {
            did_you_mean.extend(intent.did_you_mean.iter().map(|value| (*value).to_string()));
            hint = Some(intent.hint.to_string());
        } else {
            did_you_mean = nearest_flag_suggestions(wrong_flag, &valid_flags);
        }
    }
    if unknown_flag.is_some() && did_you_mean.is_empty() && hint.is_none() {
        return EnrichedCliError {
            error: err,
            details: None,
        };
    }

    let mut corrected_command = unknown_flag.as_deref().and_then(|wrong_flag| {
        if did_you_mean.len() != 1 || !did_you_mean[0].starts_with('-') {
            return None;
        }
        Some(corrected_flag_command(
            raw_args,
            wrong_flag,
            &did_you_mean[0],
        ))
    });

    if unknown_command {
        did_you_mean = crate::command_manifest::command_path_suggestions(raw_args);
        if did_you_mean.is_empty() {
            return EnrichedCliError {
                error: err,
                details: None,
            };
        }
        if let Some(suggestion) = did_you_mean.first() {
            hint = Some(format!(
                "use the nearest supported command path: {suggestion}"
            ));
            corrected_command = corrected_command_path(raw_args, suggestion);
        }
    }

    let help_command = projection
        .as_ref()
        .map(|command| command.help_command())
        .unwrap_or_else(|| "ooxml help".to_string());
    let hint = hint.unwrap_or_else(|| {
        if let Some(command) = projection.as_ref() {
            if missing_required_flags {
                let required = command.required_flags();
                let requirement = if required.is_empty() {
                    "the required arguments shown in the command usage".to_string()
                } else {
                    format!("required flags: {}", required.join(", "))
                };
                format!("{requirement}. Example: {}", command.example_command())
            } else if did_you_mean.is_empty() {
                format!(
                    "review the accepted flags and usage. Example: {}",
                    command.example_command()
                )
            } else {
                format!("did you mean {}?", did_you_mean.join(" or "))
            }
        } else if did_you_mean.is_empty() {
            "run ooxml help or ooxml --json capabilities to inspect the supported command contract"
                .to_string()
        } else {
            format!("did you mean {}?", did_you_mean.join(" or "))
        }
    });

    EnrichedCliError {
        error: err,
        details: Some(InvalidArgsDetails {
            hint,
            did_you_mean,
            valid_flags,
            help_command,
            corrected_command,
        }),
    }
}

fn unknown_flag_from_message(message: &str) -> Option<String> {
    let rest = message
        .strip_prefix("unknown flag: ")
        .or_else(|| message.strip_prefix("unknown global flag: "))?;
    let token = rest
        .split(|ch: char| ch.is_whitespace() || matches!(ch, ';' | ','))
        .next()
        .unwrap_or_default();
    (!token.is_empty()).then(|| token.to_string())
}

fn nearest_flag_suggestions(wrong_flag: &str, valid_flags: &[InvalidArgsFlag]) -> Vec<String> {
    let wrong = wrong_flag.trim_start_matches('-');
    let mut candidates = valid_flags
        .iter()
        .filter_map(|flag| {
            let candidate = flag.flag.trim_start_matches('-');
            let distance = damerau_levenshtein(wrong, candidate);
            (distance <= 2 || candidate.starts_with(wrong) || wrong.starts_with(candidate))
                .then(|| (distance, flag.flag.clone()))
        })
        .collect::<Vec<_>>();
    candidates.sort();
    candidates.dedup_by(|left, right| left.1 == right.1);
    candidates
        .into_iter()
        .take(3)
        .map(|(_, flag)| flag)
        .collect()
}

fn corrected_flag_command(raw_args: &[String], wrong_flag: &str, replacement: &str) -> String {
    let mut replaced = false;
    let args = raw_args.iter().map(|arg| {
        if replaced {
            return arg.clone();
        }
        let (flag, inline_value) = arg
            .split_once('=')
            .map(|(flag, value)| (flag, Some(value)))
            .unwrap_or((arg.as_str(), None));
        if flag != wrong_flag {
            return arg.clone();
        }
        replaced = true;
        inline_value
            .map(|value| format!("{replacement}={value}"))
            .unwrap_or_else(|| replacement.to_string())
    });
    std::iter::once("ooxml".to_string())
        .chain(args)
        .map(|arg| command_arg(&arg))
        .collect::<Vec<_>>()
        .join(" ")
}

fn corrected_command_path(raw_args: &[String], suggestion: &str) -> Option<String> {
    let suggested_tokens = suggestion
        .split_whitespace()
        .skip_while(|token| *token == "ooxml")
        .collect::<Vec<_>>();
    if suggested_tokens.is_empty() {
        return None;
    }
    let mut globals = Vec::new();
    let mut command_args = Vec::new();
    let mut index = 0;
    while index < raw_args.len() {
        match raw_args[index].as_str() {
            "--json" | "--strict" => {
                globals.push(raw_args[index].clone());
                index += 1;
            }
            "--format" | "-f" => {
                globals.push(raw_args[index].clone());
                if let Some(value) = raw_args.get(index + 1) {
                    globals.push(value.clone());
                    index += 2;
                } else {
                    index += 1;
                }
            }
            value if value.starts_with("--format=") || value.starts_with("-f=") => {
                globals.push(raw_args[index].clone());
                index += 1;
            }
            _ => {
                command_args.push(raw_args[index].clone());
                index += 1;
            }
        }
    }
    let trailing = command_args.into_iter().skip(suggested_tokens.len());
    Some(
        std::iter::once("ooxml".to_string())
            .chain(globals)
            .chain(suggested_tokens.into_iter().map(str::to_string))
            .chain(trailing)
            .map(|arg| command_arg(&arg))
            .collect::<Vec<_>>()
            .join(" "),
    )
}

fn missing_required_arguments(message: &str) -> bool {
    message.contains(" is required")
        || message.contains(" requires ")
        || message.contains("provide at least one")
        || message.contains("must specify")
        || message.contains("requires exactly")
}

fn argv_has_flag(args: &[String], name: &str) -> bool {
    args.iter().any(|arg| {
        arg == name
            || arg
                .strip_prefix(name)
                .is_some_and(|rest| rest.starts_with('='))
    })
}

pub(crate) fn damerau_levenshtein(left: &str, right: &str) -> usize {
    let left = left.chars().collect::<Vec<_>>();
    let right = right.chars().collect::<Vec<_>>();
    let mut distances = vec![vec![0; right.len() + 1]; left.len() + 1];
    for (index, row) in distances.iter_mut().enumerate() {
        row[0] = index;
    }
    for (index, distance) in distances[0].iter_mut().enumerate() {
        *distance = index;
    }
    for left_index in 1..=left.len() {
        for right_index in 1..=right.len() {
            let substitution = usize::from(left[left_index - 1] != right[right_index - 1]);
            let mut distance = (distances[left_index - 1][right_index] + 1)
                .min(distances[left_index][right_index - 1] + 1)
                .min(distances[left_index - 1][right_index - 1] + substitution);
            if left_index > 1
                && right_index > 1
                && left[left_index - 1] == right[right_index - 2]
                && left[left_index - 2] == right[right_index - 1]
            {
                distance = distance.min(distances[left_index - 2][right_index - 2] + 1);
            }
            distances[left_index][right_index] = distance;
        }
    }
    distances[left.len()][right.len()]
}

pub(crate) fn parse_validate_args(args: &[String], global_strict: bool) -> CliResult<(&str, bool)> {
    let mut strict = global_strict;
    let mut file = None;
    for arg in args {
        if arg == "--strict" {
            strict = true;
        } else if arg.starts_with("--") {
            return Err(CliError::invalid_args(format!("unknown flag: {arg}")));
        } else if file.is_some() {
            return Err(CliError::invalid_args(
                "validate accepts exactly one file argument",
            ));
        } else {
            file = Some(arg.as_str());
        }
    }
    let file =
        file.ok_or_else(|| CliError::invalid_args("validate requires exactly one file argument"))?;
    Ok((file, strict))
}

pub(crate) fn parse_string_flag(args: &[String], name: &str) -> CliResult<Option<String>> {
    let mut i = 0;
    while i < args.len() {
        if args[i] == name {
            let Some(value) = args.get(i + 1) else {
                return Err(CliError::invalid_args(format!("{name} requires a value")));
            };
            return Ok(Some(value.clone()));
        }
        if let Some(value) = args[i].strip_prefix(&format!("{name}=")) {
            return Ok(Some(value.to_string()));
        }
        i += 1;
    }
    Ok(None)
}

pub(crate) fn parse_string_flags(args: &[String], name: &str) -> CliResult<Vec<String>> {
    let mut values = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if args[i] == name {
            let Some(value) = args.get(i + 1) else {
                return Err(CliError::invalid_args(format!("{name} requires a value")));
            };
            values.push(value.clone());
            i += 2;
            continue;
        }
        if let Some(value) = args[i].strip_prefix(&format!("{name}=")) {
            values.push(value.to_string());
        }
        i += 1;
    }
    Ok(values)
}

pub(crate) fn parse_bool_flag(args: &[String], name: &str) -> CliResult<Option<bool>> {
    for arg in args {
        if arg == name {
            return Ok(Some(true));
        }
        if let Some(value) = arg.strip_prefix(&format!("{name}=")) {
            return match value {
                "true" => Ok(Some(true)),
                "false" => Ok(Some(false)),
                _ => Err(CliError::invalid_args(format!(
                    "{name} must be true or false"
                ))),
            };
        }
    }
    Ok(None)
}

pub(crate) fn reject_unknown_flags(
    args: &[String],
    value_flags: &[&str],
    bool_flags: &[&str],
) -> CliResult<()> {
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        if !arg.starts_with("--") {
            i += 1;
            continue;
        }
        if let Some((flag, value)) = arg.split_once('=') {
            if bool_flags.iter().any(|known| known == &flag) {
                if !matches!(value, "true" | "false") {
                    return Err(CliError::invalid_args(format!(
                        "{flag} must be true or false"
                    )));
                }
                i += 1;
                continue;
            }
            if value_flags.iter().any(|known| known == &flag) {
                i += 1;
                continue;
            }
        }
        if bool_flags.iter().any(|flag| flag == arg) {
            i += 1;
            continue;
        }
        if value_flags.iter().any(|flag| flag == arg) {
            if args.get(i + 1).is_none() {
                return Err(CliError::invalid_args(format!("{arg} requires a value")));
            }
            i += 2;
            continue;
        }
        return Err(CliError::invalid_args(format!("unknown flag: {arg}")));
    }
    Ok(())
}

pub(crate) fn positional_args<'a>(
    args: &'a [String],
    value_flags: &[&str],
    bool_flags: &[&str],
) -> CliResult<Vec<&'a str>> {
    let mut out = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        if !arg.starts_with("--") {
            out.push(arg.as_str());
            index += 1;
            continue;
        }
        if let Some((flag, _)) = arg.split_once('=')
            && (value_flags.iter().any(|known| known == &flag)
                || bool_flags.iter().any(|known| known == &flag))
        {
            index += 1;
            continue;
        }
        if bool_flags.iter().any(|flag| flag == arg) {
            index += 1;
            continue;
        }
        if value_flags.iter().any(|flag| flag == arg) {
            if args.get(index + 1).is_none() {
                return Err(CliError::invalid_args(format!("{arg} requires a value")));
            }
            index += 2;
            continue;
        }
        return Err(CliError::invalid_args(format!("unknown flag: {arg}")));
    }
    Ok(out)
}

pub(crate) fn output_path_arg(
    args: &[String],
    value_flags: &[&str],
    bool_flags: &[&str],
    command: &str,
) -> CliResult<String> {
    let output_flag = parse_string_flag(args, "--out")?;
    let positionals = positional_args(args, value_flags, bool_flags)?;
    match (positionals.as_slice(), output_flag.as_deref()) {
        ([positional], Some(flag_output)) if *positional != flag_output => {
            Err(CliError::invalid_args(format!(
                "{command} received conflicting output paths: positional {positional} and --out {flag_output}"
            )))
        }
        ([positional], _) => Ok((*positional).to_string()),
        ([], Some(flag_output)) => Ok(flag_output.to_string()),
        ([], None) => Err(CliError::invalid_args(format!(
            "{command} requires an output path; pass it positionally or with --out"
        ))),
        _ => Err(CliError::invalid_args(format!(
            "{command} accepts exactly one output path; pass it positionally or with --out"
        ))),
    }
}

pub(crate) fn has_flag(args: &[String], name: &str) -> bool {
    parse_bool_flag(args, name).ok().flatten().unwrap_or(false)
}

pub(crate) fn flag_present(args: &[String], name: &str) -> bool {
    has_flag(args, name)
}

pub(crate) fn value_flag_present(args: &[String], name: &str) -> bool {
    args.iter()
        .any(|arg| arg == name || arg.starts_with(&format!("{name}=")))
}

pub(crate) fn parse_u32_flag(args: &[String], name: &str) -> CliResult<Option<u32>> {
    parse_string_flag(args, name)?
        .map(|value| {
            value
                .parse::<u32>()
                .map_err(|_| CliError::invalid_args(format!("{name} must be an integer")))
        })
        .transpose()
}

pub(crate) fn parse_u32_flags(args: &[String], name: &str) -> CliResult<Vec<u32>> {
    let mut values = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if args[i] == name {
            let Some(value) = args.get(i + 1) else {
                return Err(CliError::invalid_args(format!("{name} requires a value")));
            };
            values.push(
                value
                    .parse::<u32>()
                    .map_err(|_| CliError::invalid_args(format!("{name} must be an integer")))?,
            );
            i += 2;
        } else {
            i += 1;
        }
    }
    Ok(values)
}

pub(crate) fn parse_i64_flag(args: &[String], name: &str) -> CliResult<Option<i64>> {
    parse_string_flag(args, name)?
        .map(|value| {
            value
                .parse::<i64>()
                .map_err(|_| CliError::invalid_args(format!("{name} must be an integer")))
        })
        .transpose()
}

pub(crate) fn validate_positive_i64(value: i64, name: &str) -> CliResult<()> {
    if value < 1 {
        return Err(CliError::invalid_args(format!("{name} must be >= 1")));
    }
    Ok(())
}

#[cfg(test)]
mod error_tests {
    use super::*;

    #[test]
    fn damerau_levenshtein_handles_drop_insert_and_transposition() {
        assert_eq!(damerau_levenshtein("range", "rnage"), 1);
        assert_eq!(damerau_levenshtein("range", "rage"), 1);
        assert_eq!(damerau_levenshtein("range", "ranges"), 1);
        assert_eq!(damerau_levenshtein("range", "table"), 3);
    }

    #[test]
    fn corrected_flag_commands_preserve_inline_values_and_shell_quote_arguments() {
        let args = vec![
            "xlsx".to_string(),
            "colwidths".to_string(),
            "set".to_string(),
            "book with quote's.xlsx".to_string(),
            "--rnage=A:E".to_string(),
        ];
        assert_eq!(
            corrected_flag_command(&args, "--rnage", "--range"),
            "ooxml xlsx colwidths set 'book with quote'\"'\"'s.xlsx' --range=A:E"
        );
    }
}
