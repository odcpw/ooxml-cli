mod docx;
mod pptx;
mod rules;
mod xlsx;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::{
    CliError, CliResult, InspectPackageKind, detect_inspect_package_type, has_flag,
    parse_string_flag, parse_string_flags, reject_unknown_flags, zip_entry_names,
};
use rules::{RULES, definition};

const SCHEMA_VERSION: &str = "ooxml-cli.design-check.v1";

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesignFinding {
    pub(crate) code: String,
    pub(crate) severity: String,
    pub(crate) message: String,
    pub(crate) location: Value,
    pub(crate) fix_command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) evidence: Option<Value>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct DesignConfig {
    ignore: Vec<String>,
    severity: BTreeMap<String, String>,
    thresholds: BTreeMap<String, f64>,
}

impl DesignConfig {
    pub(crate) fn threshold(&self, name: &str, fallback: f64) -> f64 {
        self.thresholds.get(name).copied().unwrap_or(fallback)
    }
}

pub(crate) fn dispatch(args: &[String]) -> CliResult<Value> {
    reject_unknown_flags(args, &["--ignore", "--config"], &["--rules"])?;
    if has_flag(args, "--rules") {
        let positionals = positional_args(args, &["--ignore", "--config"]);
        if !positionals.is_empty() {
            return Err(CliError::invalid_args(
                "design-check --rules does not accept a package path",
            ));
        }
        return Ok(json!({
            "schemaVersion": SCHEMA_VERSION,
            "rules": RULES,
        }));
    }

    let positionals = positional_args(args, &["--ignore", "--config"]);
    if positionals.len() != 1 {
        return Err(CliError::invalid_args(
            "usage: ooxml design-check <file> [--ignore CODE] [--config .ooxml-design.json], or ooxml design-check --rules",
        ));
    }
    let file = positionals[0];
    let explicit_config = parse_string_flag(args, "--config")?;
    let (mut config, config_path) = load_config(file, explicit_config.as_deref())?;
    config.ignore.extend(
        parse_string_flags(args, "--ignore")?
            .into_iter()
            .flat_map(|value| {
                value
                    .split(',')
                    .map(str::trim)
                    .filter(|code| !code.is_empty())
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
            }),
    );
    validate_config(&config)?;

    let entries = zip_entry_names(file)?;
    let family = detect_inspect_package_type(file, &entries);
    let family_name = match family {
        InspectPackageKind::Pptx => "pptx",
        InspectPackageKind::Docx => "docx",
        InspectPackageKind::Xlsx => "xlsx",
        InspectPackageKind::Unknown => {
            return Err(CliError::unsupported_type(
                "design-check supports PPTX, DOCX, and XLSX packages",
            ));
        }
    };
    let mut findings = match family {
        InspectPackageKind::Pptx => pptx::analyze(file, &entries, &config)?,
        InspectPackageKind::Docx => docx::analyze(file, &entries, &config)?,
        InspectPackageKind::Xlsx => xlsx::analyze(file, &entries, &config)?,
        InspectPackageKind::Unknown => unreachable!(),
    };
    let ignored = config.ignore.into_iter().collect::<BTreeSet<_>>();
    findings.retain(|finding| !ignored.contains(&finding.code));
    for finding in &mut findings {
        if let Some(severity) = config.severity.get(&finding.code) {
            finding.severity.clone_from(severity);
        }
    }
    findings.sort_by(|left, right| {
        rule_position(&left.code)
            .cmp(&rule_position(&right.code))
            .then_with(|| location_key(&left.location).cmp(&location_key(&right.location)))
            .then_with(|| left.message.cmp(&right.message))
    });

    let errors = findings
        .iter()
        .filter(|finding| finding.severity == "error")
        .count();
    let warnings = findings
        .iter()
        .filter(|finding| finding.severity == "warning")
        .count();
    let infos = findings
        .iter()
        .filter(|finding| finding.severity == "info")
        .count();
    let ignored_codes = ignored.into_iter().collect::<Vec<_>>();
    Ok(json!({
        "schemaVersion": SCHEMA_VERSION,
        "file": file,
        "family": family_name,
        "config": config_path.map(|path| path.to_string_lossy().into_owned()),
        "ignored": ignored_codes,
        "findings": findings,
        "summary": {
            "errors": errors,
            "warnings": warnings,
            "infos": infos,
            "total": errors + warnings + infos,
        },
        "status": if errors == 0 { "passed" } else { "failed" },
    }))
}

pub(crate) fn finding(
    code: &str,
    message: impl Into<String>,
    location: Value,
    fix_command: impl Into<String>,
    evidence: Option<Value>,
) -> DesignFinding {
    let rule = definition(code).expect("design finding must name a declared rule");
    DesignFinding {
        code: code.to_string(),
        severity: rule.severity.to_string(),
        message: message.into(),
        location,
        fix_command: fix_command.into(),
        evidence,
    }
}

pub(crate) fn fixed_output_path(file: &str, suffix: &str) -> String {
    let (directory, file_name) = file
        .rfind(['/', '\\'])
        .map_or(("", file), |index| file.split_at(index + 1));
    let file_name = if file_name.is_empty() {
        "fixed"
    } else {
        file_name
    };
    let (stem, extension) = match file_name.rfind('.') {
        Some(index) if index > 0 => (&file_name[..index], &file_name[index + 1..]),
        _ => (file_name, ""),
    };
    let name = if extension.is_empty() {
        format!("{stem}.{suffix}")
    } else {
        format!("{stem}.{suffix}.{extension}")
    };
    format!("{directory}{name}")
}

pub(crate) fn location(fields: &[(&str, Value)]) -> Value {
    let mut object = Map::new();
    for (name, value) in fields {
        object.insert((*name).to_string(), value.clone());
    }
    Value::Object(object)
}

fn load_config(file: &str, explicit: Option<&str>) -> CliResult<(DesignConfig, Option<PathBuf>)> {
    let path = if let Some(path) = explicit {
        Some(PathBuf::from(path))
    } else {
        let adjacent = Path::new(file)
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(".ooxml-design.json");
        adjacent.is_file().then_some(adjacent)
    };
    let Some(path) = path else {
        return Ok((DesignConfig::default(), None));
    };
    let bytes = fs::read(&path).map_err(|error| {
        CliError::invalid_args(format!(
            "failed to read design config {}: {error}",
            path.display()
        ))
    })?;
    let config = serde_json::from_slice(&bytes).map_err(|error| {
        CliError::invalid_args(format!("invalid design config {}: {error}", path.display()))
    })?;
    Ok((config, Some(path)))
}

fn validate_config(config: &DesignConfig) -> CliResult<()> {
    for code in config.ignore.iter().chain(config.severity.keys()) {
        if definition(code).is_none() {
            return Err(CliError::invalid_args(format!(
                "unknown design rule {code:?}; list rules with `ooxml design-check --rules`"
            )));
        }
    }
    for (code, severity) in &config.severity {
        if !matches!(severity.as_str(), "error" | "warning" | "info") {
            return Err(CliError::invalid_args(format!(
                "invalid severity {severity:?} for {code}; expected error, warning, or info"
            )));
        }
    }
    for (name, value) in &config.thresholds {
        if !value.is_finite() || *value < 0.0 {
            return Err(CliError::invalid_args(format!(
                "invalid non-negative threshold {name:?}: {value}"
            )));
        }
    }
    Ok(())
}

fn positional_args<'a>(args: &'a [String], value_flags: &[&str]) -> Vec<&'a str> {
    let mut positionals = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let arg = args[index].as_str();
        if value_flags.contains(&arg) {
            index += 2;
            continue;
        }
        if arg.starts_with("--") {
            index += 1;
            continue;
        }
        positionals.push(arg);
        index += 1;
    }
    positionals
}

fn rule_position(code: &str) -> usize {
    RULES
        .iter()
        .position(|rule| rule.code == code)
        .unwrap_or(usize::MAX)
}

fn location_key(location: &Value) -> String {
    serde_json::to_string(location).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_table_has_unique_codes_and_all_families() {
        let codes = RULES.iter().map(|rule| rule.code).collect::<BTreeSet<_>>();
        assert_eq!(codes.len(), RULES.len());
        for family in ["pptx", "docx", "xlsx"] {
            assert!(RULES.iter().any(|rule| rule.family == family));
        }
        assert!(RULES.iter().all(|rule| {
            !rule.description.is_empty() && matches!(rule.severity, "error" | "warning" | "info")
        }));
    }

    #[test]
    fn fixed_output_path_preserves_forward_slashes_for_every_family() {
        assert_eq!(
            fixed_output_path(
                "testdata/docx/scaffold-styles/dangling-style.docx",
                "style-fixed"
            ),
            "testdata/docx/scaffold-styles/dangling-style.style-fixed.docx"
        );
        assert_eq!(
            fixed_output_path("testdata/pptx/design-check/bad-deck.pptx", "design-fixed"),
            "testdata/pptx/design-check/bad-deck.design-fixed.pptx"
        );
        assert_eq!(
            fixed_output_path("testdata/xlsx/design-check/bad-sheet.xlsx", "design-fixed"),
            "testdata/xlsx/design-check/bad-sheet.design-fixed.xlsx"
        );
    }
}
