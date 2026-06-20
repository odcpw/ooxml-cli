use serde_json::Value;

use crate::cli_args::*;
use crate::cli_core::{CliError, CliResult};
use crate::xlsx_mutation::{
    XlsxCellsClearOptions, XlsxCellsSetBatchOptions, XlsxCellsSetOptions, xlsx_cells_clear,
    xlsx_cells_set, xlsx_cells_set_batch,
};
use crate::xlsx_sheets::xlsx_cells_extract;

pub(super) fn dispatch_xlsx_cells(args: &[String]) -> CliResult<Value> {
    match args {
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "cells" && verb == "extract" =>
        {
            let sheet = parse_string_flag(rest, "--sheet")?.unwrap_or_else(|| "1".to_string());
            let range = parse_string_flag(rest, "--range")?;
            let max_rows = parse_u32_flag(rest, "--max-rows")?.unwrap_or(1000);
            let max_cells = parse_u32_flag(rest, "--max-cells")?.unwrap_or(0);
            let include_empty = has_flag(rest, "--include-empty");
            xlsx_cells_extract(
                file,
                &sheet,
                range.as_deref(),
                max_rows,
                max_cells,
                include_empty,
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "cells" && verb == "set" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--cell",
                    "--ref",
                    "--value",
                    "--formula",
                    "--type",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--no-validate", "--in-place"],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let cell = parse_string_flag(rest, "--cell")?;
            let ref_ = parse_string_flag(rest, "--ref")?;
            let value = parse_string_flag(rest, "--value")?;
            let formula = parse_string_flag(rest, "--formula")?;
            let value_type = parse_string_flag(rest, "--type")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            let dry_run = has_flag(rest, "--dry-run");
            let no_validate = has_flag(rest, "--no-validate");
            let in_place = has_flag(rest, "--in-place");
            xlsx_cells_set(
                file,
                XlsxCellsSetOptions {
                    sheet: sheet.as_deref(),
                    cell: cell.as_deref(),
                    ref_: ref_.as_deref(),
                    value: value.as_deref(),
                    formula: formula.as_deref(),
                    value_type: value_type.as_deref(),
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run,
                    no_validate,
                    in_place,
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "cells" && verb == "clear" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--range",
                    "--ref",
                    "--readback-max-cells",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--no-validate", "--in-place"],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let range = parse_string_flag(rest, "--range")?;
            let ref_ = parse_string_flag(rest, "--ref")?;
            let readback_max_cells = parse_i64_flag(rest, "--readback-max-cells")?.unwrap_or(10000);
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_cells_clear(
                file,
                XlsxCellsClearOptions {
                    sheet: sheet.as_deref(),
                    range: range.as_deref(),
                    ref_: ref_.as_deref(),
                    readback_max_cells,
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "cells" && verb == "set-batch" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--cells",
                    "--cells-file",
                    "--readback-max-cells",
                    "--out",
                    "--backup",
                ],
                &["--details", "--dry-run", "--no-validate", "--in-place"],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let cells = parse_string_flag(rest, "--cells")?;
            let cells_file = parse_string_flag(rest, "--cells-file")?;
            let readback_max_cells = parse_i64_flag(rest, "--readback-max-cells")?.unwrap_or(10000);
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_cells_set_batch(
                file,
                XlsxCellsSetBatchOptions {
                    sheet: sheet.as_deref(),
                    cells: cells.as_deref(),
                    cells_file: cells_file.as_deref(),
                    details: has_flag(rest, "--details"),
                    readback_max_cells,
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
