use serde_json::Value;

use crate::cli_args::*;
use crate::cli_core::{CliError, CliResult};
use crate::{
    XlsxConditionalFormatMutationOptions, xlsx_conditional_formats_add,
    xlsx_conditional_formats_delete, xlsx_conditional_formats_list, xlsx_conditional_formats_show,
};

pub(super) fn dispatch_xlsx_conditional_formatting(args: &[String]) -> CliResult<Value> {
    match args {
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && is_conditional_formats_group(group) && verb == "list" =>
        {
            reject_unknown_flags(rest, &["--sheet", "--range"], &[])?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let range = parse_string_flag(rest, "--range")?;
            xlsx_conditional_formats_list(file, sheet.as_deref(), range.as_deref())
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && is_conditional_formats_group(group) && verb == "show" =>
        {
            reject_unknown_flags(rest, &["--sheet", "--rule"], &[])?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let rule = parse_string_flag(rest, "--rule")?
                .ok_or_else(|| CliError::invalid_args("--rule is required"))?;
            xlsx_conditional_formats_show(file, sheet.as_deref(), &rule)
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && is_conditional_formats_group(group) && verb == "add" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--range",
                    "--type",
                    "--operator",
                    "--formula",
                    "--formula2",
                    "--cfvo",
                    "--color",
                    "--icon-set",
                    "--priority",
                    "--dxf-id",
                    "--out",
                    "--backup",
                ],
                &["--stop-if-true", "--dry-run", "--no-validate", "--in-place"],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let range = parse_string_flag(rest, "--range")?;
            let rule_type = parse_string_flag(rest, "--type")?;
            let operator = parse_string_flag(rest, "--operator")?;
            let formula = parse_string_flag(rest, "--formula")?;
            let formula2 = parse_string_flag(rest, "--formula2")?;
            let cfvo = parse_string_flags(rest, "--cfvo")?;
            let colors = parse_string_flags(rest, "--color")?;
            let icon_set = parse_string_flag(rest, "--icon-set")?;
            validate_conditional_format_flags(
                rule_type.as_deref(),
                icon_set.as_deref(),
                &cfvo,
                &colors,
            )?;
            let priority = parse_i64_flag(rest, "--priority")?;
            let dxf_id = parse_i64_flag(rest, "--dxf-id")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_conditional_formats_add(
                file,
                XlsxConditionalFormatMutationOptions {
                    sheet: sheet.as_deref(),
                    range: range.as_deref(),
                    rule: None,
                    formula: formula.as_deref(),
                    rule_type: rule_type.as_deref(),
                    operator: operator.as_deref(),
                    formula2: formula2.as_deref(),
                    has_formula2: value_flag_present(rest, "--formula2"),
                    cfvo,
                    colors,
                    icon_set: icon_set.as_deref(),
                    priority,
                    stop_if_true: has_flag(rest, "--stop-if-true"),
                    has_stop_if_true: value_flag_present(rest, "--stop-if-true"),
                    dxf_id,
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && is_conditional_formats_group(group) && verb == "delete" =>
        {
            reject_unknown_flags(
                rest,
                &["--sheet", "--rule", "--out", "--backup"],
                &["--dry-run", "--no-validate", "--in-place"],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let rule = parse_string_flag(rest, "--rule")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_conditional_formats_delete(
                file,
                XlsxConditionalFormatMutationOptions {
                    sheet: sheet.as_deref(),
                    range: None,
                    rule: rule.as_deref(),
                    formula: None,
                    rule_type: None,
                    operator: None,
                    formula2: None,
                    has_formula2: false,
                    cfvo: Vec::new(),
                    colors: Vec::new(),
                    icon_set: None,
                    priority: None,
                    stop_if_true: false,
                    has_stop_if_true: false,
                    dxf_id: None,
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        _ => Err(CliError::invalid_args(format!(
            "unsupported Rust-port contract command: {}",
            args.join(" ")
        ))),
    }
}

fn is_conditional_formats_group(group: &str) -> bool {
    matches!(
        group,
        "conditional-formats" | "conditional-formatting" | "conditional-format" | "cf"
    )
}

fn validate_conditional_format_flags(
    rule_type: Option<&str>,
    icon_set: Option<&str>,
    cfvo: &[String],
    colors: &[String],
) -> CliResult<()> {
    match rule_type.map(str::trim) {
        Some("data-bar" | "dataBar") => {
            if icon_set.is_some() {
                return Err(CliError::invalid_args(
                    "--icon-set is only valid with --type icon-set",
                ));
            }
            if cfvo.len() != 2 {
                return Err(CliError::invalid_args(
                    "--type data-bar requires exactly two --cfvo values",
                ));
            }
            if colors.len() != 1 {
                return Err(CliError::invalid_args(
                    "--type data-bar requires exactly one --color value",
                ));
            }
        }
        Some("icon-set" | "iconSet") => {
            let icon_set = icon_set
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| CliError::invalid_args("--type icon-set requires --icon-set"))?;
            if !colors.is_empty() {
                return Err(CliError::invalid_args(
                    "--type icon-set does not accept --color",
                ));
            }
            let expected = expected_icon_set_cfvo_count(icon_set)?;
            if cfvo.len() != expected {
                return Err(CliError::invalid_args(format!(
                    "--type icon-set with {icon_set} requires exactly {expected} --cfvo values",
                )));
            }
        }
        _ => {
            if icon_set.is_some() {
                return Err(CliError::invalid_args(
                    "--icon-set is only valid with --type icon-set",
                ));
            }
        }
    }
    Ok(())
}

fn expected_icon_set_cfvo_count(icon_set: &str) -> CliResult<usize> {
    match icon_set.as_bytes().first().copied() {
        Some(b'3' | b'4' | b'5') => Ok((icon_set.as_bytes()[0] - b'0') as usize),
        _ => Err(CliError::invalid_args(
            "--icon-set must begin with 3, 4, or 5",
        )),
    }
}
