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
