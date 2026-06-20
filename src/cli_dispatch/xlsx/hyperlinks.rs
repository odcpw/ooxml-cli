use serde_json::Value;

use crate::cli_args::*;
use crate::cli_core::{CliError, CliResult};
use crate::{
    XlsxHyperlinkAddOptions, XlsxHyperlinkDeleteOptions, XlsxHyperlinkUpdateOptions,
    xlsx_hyperlinks_add, xlsx_hyperlinks_delete, xlsx_hyperlinks_list, xlsx_hyperlinks_show,
    xlsx_hyperlinks_update,
};

pub(super) fn dispatch_xlsx_hyperlinks(args: &[String]) -> CliResult<Value> {
    match args {
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && is_hyperlinks_group(group) && verb == "list" =>
        {
            reject_unknown_flags(rest, &["--sheet"], &["--include-broken"])?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            xlsx_hyperlinks_list(file, sheet.as_deref(), has_flag(rest, "--include-broken"))
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && is_hyperlinks_group(group) && verb == "show" =>
        {
            reject_unknown_flags(rest, &["--sheet", "--cell"], &[])?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let cell = parse_string_flag(rest, "--cell")?;
            xlsx_hyperlinks_show(file, sheet.as_deref(), cell.as_deref())
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && is_hyperlinks_group(group) && verb == "add" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--cell",
                    "--url",
                    "--location",
                    "--display",
                    "--tooltip",
                    "--out",
                    "--backup",
                ],
                &["--replace", "--dry-run", "--no-validate", "--in-place"],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let cell = parse_string_flag(rest, "--cell")?;
            let url = parse_string_flag(rest, "--url")?;
            let location = parse_string_flag(rest, "--location")?;
            let display = parse_string_flag(rest, "--display")?;
            let tooltip = parse_string_flag(rest, "--tooltip")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_hyperlinks_add(
                file,
                XlsxHyperlinkAddOptions {
                    sheet: sheet.as_deref(),
                    cell: cell.as_deref(),
                    url: url.as_deref(),
                    location: location.as_deref(),
                    display: display.as_deref(),
                    tooltip: tooltip.as_deref(),
                    replace: has_flag(rest, "--replace"),
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && is_hyperlinks_group(group) && verb == "update" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--cell",
                    "--url",
                    "--location",
                    "--display",
                    "--tooltip",
                    "--expect-url",
                    "--expect-location",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--no-validate", "--in-place"],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let cell = parse_string_flag(rest, "--cell")?;
            let url = parse_string_flag(rest, "--url")?;
            let location = parse_string_flag(rest, "--location")?;
            let display = parse_string_flag(rest, "--display")?;
            let tooltip = parse_string_flag(rest, "--tooltip")?;
            let expect_url = parse_string_flag(rest, "--expect-url")?;
            let expect_location = parse_string_flag(rest, "--expect-location")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_hyperlinks_update(
                file,
                XlsxHyperlinkUpdateOptions {
                    sheet: sheet.as_deref(),
                    cell: cell.as_deref(),
                    url: url.as_deref(),
                    set_url: value_flag_present(rest, "--url"),
                    location: location.as_deref(),
                    set_location: value_flag_present(rest, "--location"),
                    display: display.as_deref(),
                    set_display: value_flag_present(rest, "--display"),
                    tooltip: tooltip.as_deref(),
                    set_tooltip: value_flag_present(rest, "--tooltip"),
                    expect_url: expect_url.as_deref(),
                    has_expect_url: value_flag_present(rest, "--expect-url"),
                    expect_location: expect_location.as_deref(),
                    has_expect_location: value_flag_present(rest, "--expect-location"),
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && is_hyperlinks_group(group) && verb == "delete" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--sheet",
                    "--cell",
                    "--expect-url",
                    "--expect-location",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--no-validate", "--in-place"],
            )?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let cell = parse_string_flag(rest, "--cell")?;
            let expect_url = parse_string_flag(rest, "--expect-url")?;
            let expect_location = parse_string_flag(rest, "--expect-location")?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            xlsx_hyperlinks_delete(
                file,
                XlsxHyperlinkDeleteOptions {
                    sheet: sheet.as_deref(),
                    cell: cell.as_deref(),
                    expect_url: expect_url.as_deref(),
                    has_expect_url: value_flag_present(rest, "--expect-url"),
                    expect_location: expect_location.as_deref(),
                    has_expect_location: value_flag_present(rest, "--expect-location"),
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

fn is_hyperlinks_group(group: &str) -> bool {
    matches!(group, "hyperlinks" | "hyperlink" | "links")
}
