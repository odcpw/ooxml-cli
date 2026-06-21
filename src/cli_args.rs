use crate::{CliError, CliResult};

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
        if let Some((flag, _)) = arg.split_once('=') {
            if value_flags.iter().any(|known| known == &flag)
                || bool_flags.iter().any(|known| known == &flag)
            {
                index += 1;
                continue;
            }
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
