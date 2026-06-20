use serde_json::Value;

use crate::cli_args::*;
use crate::cli_core::{CliError, CliResult};
use crate::xlsx_tables::*;
use crate::{
    XlsxTablesAppendRecordsOptions, XlsxTablesAppendRowsOptions, xlsx_tables_append_records,
    xlsx_tables_append_rows,
};

pub(super) fn dispatch_xlsx_tables(args: &[String]) -> CliResult<Value> {
    match args {
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "tables" && verb == "list" =>
        {
            let sheet = parse_string_flag(rest, "--sheet")?;
            xlsx_tables_list(file, sheet.as_deref())
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "tables" && verb == "show" =>
        {
            let sheet = parse_string_flag(rest, "--sheet")?;
            let table = parse_string_flag(rest, "--table")?;
            xlsx_tables_show(file, sheet.as_deref(), table.as_deref())
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "tables" && verb == "export" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--table",
                    "--data-format",
                    "--data-out",
                    "--max-cells",
                ],
                &["--include-types", "--include-formulas"],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let table = parse_string_flag(rest, "--table")?;
            let data_format = parse_string_flag(rest, "--data-format")?;
            let data_out = parse_string_flag(rest, "--data-out")?;
            let max_cells = parse_i64_flag(rest, "--max-cells")?.unwrap_or(100000);
            let include_types = has_flag(rest, "--include-types");
            let include_formulas = has_flag(rest, "--include-formulas");
            xlsx_tables_export(
                file,
                sheet.as_deref(),
                table.as_deref(),
                XlsxTableExportOptions {
                    data_format: data_format.as_deref(),
                    data_out: data_out.as_deref(),
                    max_cells,
                    include_types,
                    include_formulas,
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "tables" && verb == "append-rows" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--table",
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
            let sheet = parse_string_flag(rest, "--sheet")?;
            let table = parse_string_flag(rest, "--table")?;
            let values = parse_string_flag(rest, "--values")?;
            let values_file = parse_string_flag(rest, "--values-file")?;
            let data_format = parse_string_flag(rest, "--data-format")?;
            let null_policy = parse_string_flag(rest, "--null-policy")?;
            let ragged = parse_string_flag(rest, "--ragged")?;
            let max_cells = parse_i64_flag(rest, "--max-cells")?.unwrap_or(100000);
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_tables_append_rows(
                file,
                XlsxTablesAppendRowsOptions {
                    sheet: sheet.as_deref(),
                    table: table.as_deref(),
                    values: values.as_deref(),
                    values_file: values_file.as_deref(),
                    data_format: data_format.as_deref(),
                    null_policy: null_policy.as_deref(),
                    null_policy_present: value_flag_present(rest, "--null-policy"),
                    ragged: ragged.as_deref(),
                    max_cells,
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                    overwrite_formulas: has_flag(rest, "--overwrite-formulas"),
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "tables" && verb == "append-records" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--table",
                    "--expect-range",
                    "--records",
                    "--records-file",
                    "--missing",
                    "--null-policy",
                    "--max-cells",
                    "--out",
                    "--backup",
                ],
                &[
                    "--ignore-extra-fields",
                    "--dry-run",
                    "--no-validate",
                    "--in-place",
                    "--overwrite-formulas",
                ],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let table = parse_string_flag(rest, "--table")?;
            let expect_range = parse_string_flag(rest, "--expect-range")?;
            let records = parse_string_flag(rest, "--records")?;
            let records_file = parse_string_flag(rest, "--records-file")?;
            let missing = parse_string_flag(rest, "--missing")?;
            let null_policy = parse_string_flag(rest, "--null-policy")?;
            let max_cells = parse_i64_flag(rest, "--max-cells")?.unwrap_or(100000);
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_tables_append_records(
                file,
                XlsxTablesAppendRecordsOptions {
                    sheet: sheet.as_deref(),
                    table: table.as_deref(),
                    expect_range: expect_range.as_deref(),
                    records: records.as_deref(),
                    records_file: records_file.as_deref(),
                    missing: missing.as_deref(),
                    null_policy: null_policy.as_deref(),
                    max_cells,
                    ignore_extra_fields: has_flag(rest, "--ignore-extra-fields"),
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                    overwrite_formulas: has_flag(rest, "--overwrite-formulas"),
                },
            )
        }
        _ => Err(CliError::invalid_args(format!(
            "unsupported Rust-port contract command: {}",
            args.join(" ")
        ))),
    }
}
