mod docx;

use serde_json::{Value, json};

use crate::capabilities;
use crate::cli_args::*;
use crate::cli_core::{CliError, CliResult, GlobalFlags};
use crate::inspect::inspect;
use crate::pptx_mutation::*;
use crate::pptx_readback::*;
use crate::pptx_render::pptx_render;
use crate::validation::validate;
use crate::vba::*;
use crate::verify::verify;
use crate::xlsx_freeze::*;
use crate::xlsx_metadata::*;
use crate::xlsx_mutation::*;
use crate::xlsx_names::*;
use crate::xlsx_ranges::*;
use crate::xlsx_sheets::*;
use crate::xlsx_tables::*;
pub(crate) fn dispatch(flags: &GlobalFlags, args: &[String]) -> CliResult<Value> {
    match args {
        [cmd] if cmd == "version" => Ok(json!({"tool": "ooxml", "version": "0.0.1"})),
        [cmd, rest @ ..] if cmd == "capabilities" => capabilities::capabilities(rest),
        [cmd, file] if cmd == "inspect" => inspect(file),
        [cmd, rest @ ..] if cmd == "validate" => {
            let (file, strict) = parse_validate_args(rest, flags.strict)?;
            validate(file, strict)
        }
        [cmd, file, rest @ ..] if cmd == "verify" => verify(file, rest),
        [family, verb, file] if family == "vba" && verb == "inspect" => vba_inspect(file),
        [family, verb, file, rest @ ..] if family == "vba" && verb == "extract-bin" => {
            reject_unknown_flags(rest, &["--out"], &[])?;
            let out = parse_string_flag(rest, "--out")?
                .ok_or_else(|| CliError::invalid_args("--out is required"))?;
            vba_extract_bin(file, &out)
        }
        [family, verb, file, rest @ ..] if family == "vba" && verb == "attach" => {
            reject_unknown_flags(
                rest,
                &["--bin", "--out", "--backup"],
                &[
                    "--allow-host-family-risk",
                    "--dry-run",
                    "--no-validate",
                    "--in-place",
                ],
            )?;
            let bin = parse_string_flag(rest, "--bin")?
                .ok_or_else(|| CliError::invalid_args("--bin is required"))?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            vba_attach(
                file,
                &bin,
                VbaMutationOptions {
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, verb, file, rest @ ..] if family == "vba" && verb == "remove" => {
            reject_unknown_flags(
                rest,
                &["--out", "--backup"],
                &["--dry-run", "--no-validate", "--in-place"],
            )?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            vba_remove(
                file,
                VbaMutationOptions {
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, ..] if family == "docx" => docx::dispatch_docx(args),
        [family, verb, file, rest @ ..] if family == "pptx" && verb == "render" => {
            pptx_render(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "slides" && verb == "show" =>
        {
            let slide = parse_u32_flag(rest, "--slide")?.unwrap_or(1);
            pptx_slide_show(file, slide)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "slides" && verb == "selectors" =>
        {
            let slide = parse_u32_flag(rest, "--slide")?
                .ok_or_else(|| CliError::invalid_args("--slide is required"))?;
            pptx_slide_selectors(file, slide)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "shapes" && verb == "show" =>
        {
            let slide = parse_u32_flag(rest, "--slide")?
                .ok_or_else(|| CliError::invalid_args("required flag(s) \"slide\" not set"))?;
            let include_text = has_flag(rest, "--include-text");
            let include_bounds = has_flag(rest, "--include-bounds");
            pptx_shapes_show(file, slide, include_text, include_bounds)
        }
        [family, group, verb, file] if family == "pptx" && group == "slides" && verb == "list" => {
            pptx_slides_list(file)
        }
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
        [family, group, subgroup, verb, file]
            if family == "xlsx"
                && group == "workbook"
                && subgroup == "metadata"
                && verb == "inspect" =>
        {
            xlsx_workbook_metadata_inspect(file)
        }
        [family, group, subgroup, verb, file, rest @ ..]
            if family == "xlsx"
                && group == "workbook"
                && subgroup == "metadata"
                && verb == "update" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--title",
                    "--subject",
                    "--creator",
                    "--keywords",
                    "--description",
                    "--last-modified-by",
                    "--category",
                    "--company",
                    "--manager",
                    "--calc-mode",
                    "--expect-title",
                    "--expect-subject",
                    "--expect-creator",
                    "--expect-keywords",
                    "--expect-description",
                    "--expect-last-modified-by",
                    "--expect-category",
                    "--expect-company",
                    "--expect-manager",
                    "--out",
                    "--backup",
                ],
                &[
                    "--full-calc-on-load",
                    "--dry-run",
                    "--no-validate",
                    "--in-place",
                ],
            )?;
            let title = parse_string_flag(rest, "--title")?;
            let subject = parse_string_flag(rest, "--subject")?;
            let creator = parse_string_flag(rest, "--creator")?;
            let keywords = parse_string_flag(rest, "--keywords")?;
            let description = parse_string_flag(rest, "--description")?;
            let last_modified_by = parse_string_flag(rest, "--last-modified-by")?;
            let category = parse_string_flag(rest, "--category")?;
            let company = parse_string_flag(rest, "--company")?;
            let manager = parse_string_flag(rest, "--manager")?;
            let calc_mode = parse_string_flag(rest, "--calc-mode")?;
            let expect_title = parse_string_flag(rest, "--expect-title")?;
            let expect_subject = parse_string_flag(rest, "--expect-subject")?;
            let expect_creator = parse_string_flag(rest, "--expect-creator")?;
            let expect_keywords = parse_string_flag(rest, "--expect-keywords")?;
            let expect_description = parse_string_flag(rest, "--expect-description")?;
            let expect_last_modified_by = parse_string_flag(rest, "--expect-last-modified-by")?;
            let expect_category = parse_string_flag(rest, "--expect-category")?;
            let expect_company = parse_string_flag(rest, "--expect-company")?;
            let expect_manager = parse_string_flag(rest, "--expect-manager")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            let full_calc_on_load = parse_bool_flag(rest, "--full-calc-on-load")?;
            xlsx_workbook_metadata_update(
                file,
                XlsxWorkbookMetadataUpdateOptions {
                    title: title.as_deref(),
                    subject: subject.as_deref(),
                    creator: creator.as_deref(),
                    keywords: keywords.as_deref(),
                    description: description.as_deref(),
                    last_modified_by: last_modified_by.as_deref(),
                    category: category.as_deref(),
                    company: company.as_deref(),
                    manager: manager.as_deref(),
                    calc_mode: calc_mode.as_deref(),
                    full_calc_on_load,
                    expect_title: expect_title.as_deref(),
                    expect_subject: expect_subject.as_deref(),
                    expect_creator: expect_creator.as_deref(),
                    expect_keywords: expect_keywords.as_deref(),
                    expect_description: expect_description.as_deref(),
                    expect_last_modified_by: expect_last_modified_by.as_deref(),
                    expect_category: expect_category.as_deref(),
                    expect_company: expect_company.as_deref(),
                    expect_manager: expect_manager.as_deref(),
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
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
            if family == "xlsx" && group == "freeze" && verb == "show" =>
        {
            reject_unknown_flags(rest, &["--sheet"], &[])?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            xlsx_freeze_show(file, sheet.as_deref())
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "freeze" && verb == "set" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--rows",
                    "--cols",
                    "--expect-state",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--no-validate", "--in-place"],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let rows = parse_i64_flag(rest, "--rows")?.unwrap_or(0);
            let cols = parse_i64_flag(rest, "--cols")?.unwrap_or(0);
            let expect_state = parse_string_flag(rest, "--expect-state")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_freeze_set(
                file,
                XlsxFreezeMutationOptions {
                    sheet: sheet.as_deref(),
                    rows,
                    cols,
                    expect_state: expect_state.as_deref(),
                    expect_state_present: value_flag_present(rest, "--expect-state"),
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "freeze" && verb == "clear" =>
        {
            reject_unknown_flags(
                rest,
                &["--sheet", "--expect-state", "--out", "--backup"],
                &["--dry-run", "--no-validate", "--in-place"],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let expect_state = parse_string_flag(rest, "--expect-state")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_freeze_clear(
                file,
                XlsxFreezeMutationOptions {
                    sheet: sheet.as_deref(),
                    rows: 0,
                    cols: 0,
                    expect_state: expect_state.as_deref(),
                    expect_state_present: value_flag_present(rest, "--expect-state"),
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, group, verb, file] if family == "xlsx" && group == "sheets" && verb == "list" => {
            xlsx_sheets_list(file)
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "sheets" && verb == "show" =>
        {
            let sheet = parse_string_flag(rest, "--sheet")?;
            xlsx_sheets_show(file, sheet.as_deref())
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx"
                && (group == "names" || group == "defined-names")
                && verb == "list" =>
        {
            reject_unknown_flags(rest, &["--scope-sheet"], &[])?;
            let scope_sheet = parse_string_flag(rest, "--scope-sheet")?;
            xlsx_names_list(file, scope_sheet.as_deref())
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx"
                && (group == "names" || group == "defined-names")
                && verb == "add" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--name",
                    "--ref",
                    "--sheet",
                    "--range",
                    "--scope-sheet",
                    "--comment",
                    "--out",
                    "--backup",
                ],
                &["--hidden", "--dry-run", "--no-validate", "--in-place"],
            )?;
            let name = parse_string_flag(rest, "--name")?;
            let ref_ = parse_string_flag(rest, "--ref")?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let range = parse_string_flag(rest, "--range")?;
            let scope_sheet = parse_string_flag(rest, "--scope-sheet")?;
            let comment = parse_string_flag(rest, "--comment")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_names_add(
                file,
                XlsxNameMutationOptions {
                    name: name.as_deref(),
                    new_name: None,
                    ref_: ref_.as_deref(),
                    sheet: sheet.as_deref(),
                    range: range.as_deref(),
                    scope_sheet: scope_sheet.as_deref(),
                    expect_ref: None,
                    hidden: has_flag(rest, "--hidden"),
                    comment: comment.as_deref(),
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx"
                && (group == "names" || group == "defined-names")
                && verb == "update" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--name",
                    "--ref",
                    "--sheet",
                    "--range",
                    "--scope-sheet",
                    "--expect-ref",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--no-validate", "--in-place"],
            )?;
            let name = parse_string_flag(rest, "--name")?;
            let ref_ = parse_string_flag(rest, "--ref")?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let range = parse_string_flag(rest, "--range")?;
            let scope_sheet = parse_string_flag(rest, "--scope-sheet")?;
            let expect_ref = parse_string_flag(rest, "--expect-ref")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_names_update(
                file,
                XlsxNameMutationOptions {
                    name: name.as_deref(),
                    new_name: None,
                    ref_: ref_.as_deref(),
                    sheet: sheet.as_deref(),
                    range: range.as_deref(),
                    scope_sheet: scope_sheet.as_deref(),
                    expect_ref: expect_ref.as_deref(),
                    hidden: false,
                    comment: None,
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx"
                && (group == "names" || group == "defined-names")
                && verb == "rename" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--name",
                    "--new-name",
                    "--scope-sheet",
                    "--expect-ref",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--no-validate", "--in-place"],
            )?;
            let name = parse_string_flag(rest, "--name")?;
            let new_name = parse_string_flag(rest, "--new-name")?;
            let scope_sheet = parse_string_flag(rest, "--scope-sheet")?;
            let expect_ref = parse_string_flag(rest, "--expect-ref")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_names_rename(
                file,
                XlsxNameMutationOptions {
                    name: name.as_deref(),
                    new_name: new_name.as_deref(),
                    ref_: None,
                    sheet: None,
                    range: None,
                    scope_sheet: scope_sheet.as_deref(),
                    expect_ref: expect_ref.as_deref(),
                    hidden: false,
                    comment: None,
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx"
                && (group == "names" || group == "defined-names")
                && verb == "delete" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--name",
                    "--scope-sheet",
                    "--expect-ref",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--no-validate", "--in-place"],
            )?;
            let name = parse_string_flag(rest, "--name")?;
            let scope_sheet = parse_string_flag(rest, "--scope-sheet")?;
            let expect_ref = parse_string_flag(rest, "--expect-ref")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_names_delete(
                file,
                XlsxNameMutationOptions {
                    name: name.as_deref(),
                    new_name: None,
                    ref_: None,
                    sheet: None,
                    range: None,
                    scope_sheet: scope_sheet.as_deref(),
                    expect_ref: expect_ref.as_deref(),
                    hidden: false,
                    comment: None,
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx"
                && (group == "names" || group == "defined-names")
                && verb == "show" =>
        {
            reject_unknown_flags(rest, &["--name", "--scope-sheet"], &[])?;
            let name = parse_string_flag(rest, "--name")?;
            let scope_sheet = parse_string_flag(rest, "--scope-sheet")?;
            xlsx_names_show(file, name.as_deref().unwrap_or(""), scope_sheet.as_deref())
        }
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
            if family == "pptx" && group == "extract" && verb == "text" =>
        {
            reject_unknown_flags(rest, &["--slide"], &[])?;
            pptx_extract_text(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "extract" && verb == "notes" =>
        {
            reject_unknown_flags(rest, &["--slide"], &[])?;
            pptx_extract_notes(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "notes" && verb == "show" =>
        {
            reject_unknown_flags(rest, &["--slide"], &[])?;
            let slide = parse_u32_flag(rest, "--slide")?
                .ok_or_else(|| CliError::invalid_args("--slide is required"))?;
            pptx_notes_show(file, slide)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "comments" && verb == "list" =>
        {
            reject_unknown_flags(rest, &["--slide", "--comment-id"], &[])?;
            let slide = parse_i64_flag(rest, "--slide")?;
            let comment_id = parse_i64_flag(rest, "--comment-id")?;
            if let Some(slide) = slide
                && slide < 1
            {
                return Err(CliError::invalid_args("--slide must be >= 1"));
            }
            if comment_id.is_some() && slide.is_none() {
                return Err(CliError::invalid_args("--comment-id requires --slide"));
            }
            pptx_comments_list(file, slide.map(|value| value as u32), comment_id)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "masters" && verb == "list" =>
        {
            reject_unknown_flags(rest, &[], &[])?;
            pptx_masters_list(file)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "masters" && verb == "show" =>
        {
            reject_unknown_flags(rest, &["--master"], &[])?;
            let master = parse_i64_flag(rest, "--master")?.unwrap_or(1);
            pptx_masters_show(file, master)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "layouts" && verb == "list" =>
        {
            reject_unknown_flags(rest, &["--master"], &[])?;
            let master = parse_i64_flag(rest, "--master")?;
            if let Some(master) = master
                && master < 0
            {
                return Err(CliError::invalid_args("--master must be >= 0"));
            }
            pptx_layouts_list(file, master.map(|value| value as u32))
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "layouts" && verb == "show" =>
        {
            reject_unknown_flags(rest, &["--layout"], &[])?;
            let layout = parse_string_flag(rest, "--layout")?
                .ok_or_else(|| CliError::invalid_args("--layout flag is required"))?;
            pptx_layouts_show(file, &layout)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "tables" && verb == "show" =>
        {
            reject_unknown_flags(rest, &["--slide", "--table-id", "--target"], &["--details"])?;
            let slide = parse_i64_flag(rest, "--slide")?.unwrap_or(0);
            if slide < 1 {
                return Err(CliError::invalid_args("--slide must be >= 1"));
            }
            let table_id = parse_i64_flag(rest, "--table-id")?.unwrap_or(0);
            if table_id < 0 {
                return Err(CliError::invalid_args(
                    "--table-id must be a positive integer",
                ));
            }
            let target = parse_string_flag(rest, "--target")?;
            if table_id > 0 && target.as_deref().unwrap_or_default() != "" {
                return Err(CliError::invalid_args(
                    "specify only one of --target or --table-id",
                ));
            }
            pptx_tables_show(
                file,
                slide as u32,
                table_id as u32,
                target.as_deref(),
                has_flag(rest, "--details"),
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "replace" && verb == "text" =>
        {
            pptx_replace_text(file, rest)
        }
        _ => Err(CliError::invalid_args(format!(
            "unsupported Rust-port contract command: {}",
            args.join(" ")
        ))),
    }
}

pub(crate) fn require_docx_block_hash(value: &str) -> CliResult<()> {
    if value.trim().is_empty() {
        return Err(CliError::invalid_args("--expect-hash is required"));
    }
    let Some(hex) = value.strip_prefix("sha256:") else {
        return Err(CliError::invalid_args(
            "--expect-hash must match sha256:<64 lowercase hex chars> from docx blocks",
        ));
    };
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|ch| ch.is_ascii_hexdigit() && !ch.is_ascii_uppercase())
    {
        return Err(CliError::invalid_args(
            "--expect-hash must match sha256:<64 lowercase hex chars> from docx blocks",
        ));
    }
    Ok(())
}
