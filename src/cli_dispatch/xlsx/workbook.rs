use serde_json::Value;

use crate::cli_args::*;
use crate::cli_core::{CliError, CliResult};
use crate::{
    XlsxWorkbookMetadataUpdateOptions, xlsx_workbook_metadata_inspect,
    xlsx_workbook_metadata_update,
};

pub(super) fn dispatch_xlsx_workbook(args: &[String]) -> CliResult<Value> {
    match args {
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
        _ => Err(CliError::invalid_args(format!(
            "unsupported Rust-port contract command: {}",
            args.join(" ")
        ))),
    }
}
