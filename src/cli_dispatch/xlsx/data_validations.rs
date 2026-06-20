use serde_json::Value;

use crate::cli_args::*;
use crate::cli_core::{CliError, CliResult};
use crate::{
    XlsxDataValidationFields, XlsxDataValidationMutationOptions, xlsx_data_validations_create,
    xlsx_data_validations_delete, xlsx_data_validations_list, xlsx_data_validations_show,
    xlsx_data_validations_update,
};

pub(super) fn dispatch_xlsx_data_validations(args: &[String]) -> CliResult<Value> {
    match args {
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && is_data_validations_group(group) && verb == "list" =>
        {
            reject_unknown_flags(rest, &["--sheet"], &[])?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            xlsx_data_validations_list(file, sheet.as_deref())
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && is_data_validations_group(group) && verb == "show" =>
        {
            reject_unknown_flags(rest, &["--sheet", "--range"], &[])?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let range = parse_string_flag(rest, "--range")?
                .ok_or_else(|| CliError::invalid_args("--range is required"))?;
            xlsx_data_validations_show(file, sheet.as_deref(), &range)
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && is_data_validations_group(group) && verb == "create" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--range",
                    "--type",
                    "--list-values",
                    "--list-range",
                    "--operator",
                    "--formula1",
                    "--formula2",
                    "--input-title",
                    "--input-message",
                    "--error-title",
                    "--error-message",
                    "--error-style",
                    "--out",
                    "--backup",
                ],
                &[
                    "--allow-blank",
                    "--show-input-message",
                    "--show-error-message",
                    "--dry-run",
                    "--no-validate",
                    "--in-place",
                ],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let range = parse_string_flag(rest, "--range")?;
            let validation_type = parse_string_flag(rest, "--type")?;
            let list_values = parse_string_flag(rest, "--list-values")?;
            let list_range = parse_string_flag(rest, "--list-range")?;
            let operator = parse_string_flag(rest, "--operator")?;
            let formula1 = parse_string_flag(rest, "--formula1")?;
            let formula2 = parse_string_flag(rest, "--formula2")?;
            let input_title = parse_string_flag(rest, "--input-title")?;
            let input_message = parse_string_flag(rest, "--input-message")?;
            let error_title = parse_string_flag(rest, "--error-title")?;
            let error_message = parse_string_flag(rest, "--error-message")?;
            let error_style = parse_string_flag(rest, "--error-style")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_data_validations_create(
                file,
                XlsxDataValidationMutationOptions {
                    sheet: sheet.as_deref(),
                    range: range.as_deref(),
                    fields: xlsx_data_validation_fields_from_flags(
                        rest,
                        validation_type.as_deref(),
                        operator.as_deref(),
                        formula1.as_deref(),
                        formula2.as_deref(),
                        list_values.as_deref(),
                        list_range.as_deref(),
                        input_title.as_deref(),
                        input_message.as_deref(),
                        error_title.as_deref(),
                        error_message.as_deref(),
                        error_style.as_deref(),
                    ),
                    expect_type: None,
                    expect_type_present: false,
                    expect_formula1: None,
                    expect_formula1_present: false,
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && is_data_validations_group(group) && verb == "update" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--range",
                    "--type",
                    "--list-values",
                    "--list-range",
                    "--operator",
                    "--formula1",
                    "--formula2",
                    "--input-title",
                    "--input-message",
                    "--error-title",
                    "--error-message",
                    "--error-style",
                    "--expect-type",
                    "--expect-formula1",
                    "--out",
                    "--backup",
                ],
                &[
                    "--allow-blank",
                    "--show-input-message",
                    "--show-error-message",
                    "--dry-run",
                    "--no-validate",
                    "--in-place",
                ],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let range = parse_string_flag(rest, "--range")?;
            let validation_type = parse_string_flag(rest, "--type")?;
            let list_values = parse_string_flag(rest, "--list-values")?;
            let list_range = parse_string_flag(rest, "--list-range")?;
            let operator = parse_string_flag(rest, "--operator")?;
            let formula1 = parse_string_flag(rest, "--formula1")?;
            let formula2 = parse_string_flag(rest, "--formula2")?;
            let input_title = parse_string_flag(rest, "--input-title")?;
            let input_message = parse_string_flag(rest, "--input-message")?;
            let error_title = parse_string_flag(rest, "--error-title")?;
            let error_message = parse_string_flag(rest, "--error-message")?;
            let error_style = parse_string_flag(rest, "--error-style")?;
            let expect_type = parse_string_flag(rest, "--expect-type")?;
            let expect_formula1 = parse_string_flag(rest, "--expect-formula1")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_data_validations_update(
                file,
                XlsxDataValidationMutationOptions {
                    sheet: sheet.as_deref(),
                    range: range.as_deref(),
                    fields: xlsx_data_validation_fields_from_flags(
                        rest,
                        validation_type.as_deref(),
                        operator.as_deref(),
                        formula1.as_deref(),
                        formula2.as_deref(),
                        list_values.as_deref(),
                        list_range.as_deref(),
                        input_title.as_deref(),
                        input_message.as_deref(),
                        error_title.as_deref(),
                        error_message.as_deref(),
                        error_style.as_deref(),
                    ),
                    expect_type: expect_type.as_deref(),
                    expect_type_present: value_flag_present(rest, "--expect-type"),
                    expect_formula1: expect_formula1.as_deref(),
                    expect_formula1_present: value_flag_present(rest, "--expect-formula1"),
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && is_data_validations_group(group) && verb == "delete" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--range",
                    "--expect-type",
                    "--expect-formula1",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--no-validate", "--in-place"],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let range = parse_string_flag(rest, "--range")?;
            let expect_type = parse_string_flag(rest, "--expect-type")?;
            let expect_formula1 = parse_string_flag(rest, "--expect-formula1")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_data_validations_delete(
                file,
                XlsxDataValidationMutationOptions {
                    sheet: sheet.as_deref(),
                    range: range.as_deref(),
                    fields: empty_xlsx_data_validation_fields(),
                    expect_type: expect_type.as_deref(),
                    expect_type_present: value_flag_present(rest, "--expect-type"),
                    expect_formula1: expect_formula1.as_deref(),
                    expect_formula1_present: value_flag_present(rest, "--expect-formula1"),
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

fn is_data_validations_group(group: &str) -> bool {
    matches!(
        group,
        "data-validations" | "data-validation" | "datavalidations" | "dv"
    )
}

#[allow(clippy::too_many_arguments)]
fn xlsx_data_validation_fields_from_flags<'a>(
    args: &[String],
    validation_type: Option<&'a str>,
    operator: Option<&'a str>,
    formula1: Option<&'a str>,
    formula2: Option<&'a str>,
    list_values: Option<&'a str>,
    list_range: Option<&'a str>,
    input_title: Option<&'a str>,
    input_message: Option<&'a str>,
    error_title: Option<&'a str>,
    error_message: Option<&'a str>,
    error_style: Option<&'a str>,
) -> XlsxDataValidationFields<'a> {
    XlsxDataValidationFields {
        validation_type,
        operator,
        formula1,
        formula2,
        list_values,
        list_range,
        allow_blank: has_flag(args, "--allow-blank"),
        show_input_message: has_flag(args, "--show-input-message"),
        show_error_message: has_flag(args, "--show-error-message"),
        prompt_title: input_title,
        prompt: input_message,
        error_title,
        error: error_message,
        error_style,
        set_type: value_flag_present(args, "--type"),
        set_operator: value_flag_present(args, "--operator"),
        set_formula1: value_flag_present(args, "--formula1"),
        set_formula2: value_flag_present(args, "--formula2"),
        set_list_values: value_flag_present(args, "--list-values"),
        set_list_range: value_flag_present(args, "--list-range"),
        set_allow_blank: value_flag_present(args, "--allow-blank"),
        set_show_input_message: value_flag_present(args, "--show-input-message"),
        set_show_error_message: value_flag_present(args, "--show-error-message"),
        set_prompt_title: value_flag_present(args, "--input-title"),
        set_prompt: value_flag_present(args, "--input-message"),
        set_error_title: value_flag_present(args, "--error-title"),
        set_error: value_flag_present(args, "--error-message"),
        set_error_style: value_flag_present(args, "--error-style"),
    }
}

fn empty_xlsx_data_validation_fields<'a>() -> XlsxDataValidationFields<'a> {
    XlsxDataValidationFields {
        validation_type: None,
        operator: None,
        formula1: None,
        formula2: None,
        list_values: None,
        list_range: None,
        allow_blank: false,
        show_input_message: false,
        show_error_message: false,
        prompt_title: None,
        prompt: None,
        error_title: None,
        error: None,
        error_style: None,
        set_type: false,
        set_operator: false,
        set_formula1: false,
        set_formula2: false,
        set_list_values: false,
        set_list_range: false,
        set_allow_blank: false,
        set_show_input_message: false,
        set_show_error_message: false,
        set_prompt_title: false,
        set_prompt: false,
        set_error_title: false,
        set_error: false,
        set_error_style: false,
    }
}
