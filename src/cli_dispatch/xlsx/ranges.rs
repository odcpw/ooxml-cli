use serde_json::Value;

use crate::cli_args::*;
use crate::cli_core::{CliError, CliResult};
use crate::xlsx_mutation::*;
use crate::xlsx_ranges::*;

pub(super) fn dispatch_xlsx_ranges(args: &[String]) -> CliResult<Value> {
    match args {
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "ranges" && verb == "export" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--range",
                    "--data-format",
                    "--data-out",
                    "--max-cells",
                ],
                &["--include-types", "--include-formulas", "--include-formats"],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?.unwrap_or_else(|| "1".to_string());
            let range = parse_string_flag(rest, "--range")?
                .ok_or_else(|| CliError::invalid_args("--range is required"))?;
            let data_format = parse_string_flag(rest, "--data-format")?;
            require_json_data_format(data_format.as_deref())?;
            let data_out = parse_string_flag(rest, "--data-out")?;
            let max_cells = parse_i64_flag(rest, "--max-cells")?.unwrap_or(100000);
            let include_types = has_flag(rest, "--include-types");
            let include_formulas = has_flag(rest, "--include-formulas");
            let include_formats = has_flag(rest, "--include-formats");
            xlsx_range_export_with_options(
                file,
                &sheet,
                &range,
                XlsxRangeExportOptions {
                    include_types,
                    include_formulas,
                    include_formats,
                    data_out: data_out.as_deref(),
                    max_cells,
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "ranges" && verb == "set" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--range",
                    "--anchor",
                    "--values",
                    "--values-file",
                    "--data-format",
                    "--null-policy",
                    "--ragged",
                    "--max-cells",
                    "--out",
                    "--backup",
                ],
                &[
                    "--dry-run",
                    "--no-validate",
                    "--in-place",
                    "--overwrite-formulas",
                ],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?
                .ok_or_else(|| CliError::invalid_args("--sheet is required for range commands"))?;
            let range = parse_string_flag(rest, "--range")?;
            let anchor = parse_string_flag(rest, "--anchor")?;
            let values = parse_string_flag(rest, "--values")?;
            let values_file = parse_string_flag(rest, "--values-file")?;
            let data_format = parse_string_flag(rest, "--data-format")?;
            let null_policy = parse_string_flag(rest, "--null-policy")?;
            let ragged = parse_string_flag(rest, "--ragged")?;
            let max_cells = parse_i64_flag(rest, "--max-cells")?.unwrap_or(100000);
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            let dry_run = has_flag(rest, "--dry-run");
            let no_validate = has_flag(rest, "--no-validate");
            let in_place = has_flag(rest, "--in-place");
            let overwrite_formulas = has_flag(rest, "--overwrite-formulas");
            xlsx_ranges_set(
                file,
                XlsxRangesSetOptions {
                    sheet: &sheet,
                    range: range.as_deref(),
                    anchor: anchor.as_deref(),
                    values: values.as_deref(),
                    values_file: values_file.as_deref(),
                    data_format: data_format.as_deref(),
                    null_policy: null_policy.as_deref(),
                    ragged: ragged.as_deref(),
                    max_cells,
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run,
                    no_validate,
                    in_place,
                    overwrite_formulas,
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "ranges" && verb == "set-format" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--range",
                    "--preset",
                    "--format-code",
                    "--decimals",
                    "--currency-symbol",
                    "--max-cells",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--no-validate", "--in-place"],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?
                .ok_or_else(|| CliError::invalid_args("--sheet is required for range commands"))?;
            let range = parse_string_flag(rest, "--range")?
                .ok_or_else(|| CliError::invalid_args("--range is required"))?;
            let preset = parse_string_flag(rest, "--preset")?;
            let format_code = parse_string_flag(rest, "--format-code")?;
            let decimals = parse_i64_flag(rest, "--decimals")?.unwrap_or(2);
            let currency_symbol = parse_string_flag(rest, "--currency-symbol")?;
            let max_cells = parse_i64_flag(rest, "--max-cells")?.unwrap_or(100000);
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            let dry_run = has_flag(rest, "--dry-run");
            let no_validate = has_flag(rest, "--no-validate");
            let in_place = has_flag(rest, "--in-place");
            xlsx_ranges_set_format(
                file,
                XlsxRangesSetFormatOptions {
                    sheet: &sheet,
                    range: &range,
                    preset: preset.as_deref(),
                    format_code: format_code.as_deref(),
                    decimals,
                    currency_symbol: currency_symbol.as_deref(),
                    max_cells,
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run,
                    no_validate,
                    in_place,
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "ranges" && verb == "set-style" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--range",
                    "--font-name",
                    "--font-size",
                    "--font-color",
                    "--fill-color",
                    "--border-style",
                    "--border-color",
                    "--alignment-horizontal",
                    "--alignment-vertical",
                    "--max-cells",
                    "--out",
                    "--backup",
                ],
                &[
                    "--font-bold",
                    "--font-italic",
                    "--font-underline",
                    "--border-top",
                    "--border-bottom",
                    "--border-left",
                    "--border-right",
                    "--alignment-wrap-text",
                    "--dry-run",
                    "--no-validate",
                    "--in-place",
                ],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?
                .ok_or_else(|| CliError::invalid_args("--sheet is required for range commands"))?;
            let range = parse_string_flag(rest, "--range")?
                .ok_or_else(|| CliError::invalid_args("--range is required"))?;
            let font_name = parse_string_flag(rest, "--font-name")?;
            let font_size = parse_string_flag(rest, "--font-size")?
                .map(|value| {
                    value
                        .parse::<f64>()
                        .map_err(|_| CliError::invalid_args("--font-size must be a number"))
                })
                .transpose()?;
            let font_color = parse_string_flag(rest, "--font-color")?;
            let fill_color = parse_string_flag(rest, "--fill-color")?;
            let border_style = parse_string_flag(rest, "--border-style")?;
            let border_color = parse_string_flag(rest, "--border-color")?;
            let alignment_horizontal = parse_string_flag(rest, "--alignment-horizontal")?;
            let alignment_vertical = parse_string_flag(rest, "--alignment-vertical")?;
            let max_cells = parse_i64_flag(rest, "--max-cells")?.unwrap_or(100000);
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_ranges_set_style(
                file,
                XlsxRangesSetStyleOptions {
                    sheet: &sheet,
                    range: &range,
                    font_name: font_name.as_deref(),
                    font_size,
                    font_bold: parse_bool_flag(rest, "--font-bold")?,
                    font_italic: parse_bool_flag(rest, "--font-italic")?,
                    font_underline: parse_bool_flag(rest, "--font-underline")?,
                    font_color: font_color.as_deref(),
                    fill_color: fill_color.as_deref(),
                    border_style: border_style.as_deref(),
                    border_color: border_color.as_deref(),
                    border_top: parse_bool_flag(rest, "--border-top")?,
                    border_bottom: parse_bool_flag(rest, "--border-bottom")?,
                    border_left: parse_bool_flag(rest, "--border-left")?,
                    border_right: parse_bool_flag(rest, "--border-right")?,
                    alignment_horizontal: alignment_horizontal.as_deref(),
                    alignment_vertical: alignment_vertical.as_deref(),
                    alignment_wrap_text: parse_bool_flag(rest, "--alignment-wrap-text")?,
                    max_cells,
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
