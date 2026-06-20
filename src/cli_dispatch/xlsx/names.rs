use serde_json::Value;

use crate::cli_args::*;
use crate::cli_core::{CliError, CliResult};
use crate::xlsx_names::*;

pub(super) fn dispatch_xlsx_names(args: &[String]) -> CliResult<Value> {
    match args {
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
        _ => Err(CliError::invalid_args(format!(
            "unsupported Rust-port contract command: {}",
            args.join(" ")
        ))),
    }
}
