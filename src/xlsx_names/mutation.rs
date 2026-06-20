use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::{
    CliError, CliResult, RangeBounds, WorkbookSheet, col_name, copy_zip_with_part_overrides,
    parse_cell_ref, parse_cli_range, resolve_sheet, validate, validate_xlsx_mutation_output_flags,
    xlsx_ranges_set_temp_path,
};

use super::model::{
    XlsxDefinedName, XlsxNameMutationOptions, apply_defined_name_sheet_context,
    defined_name_scope_text, renumber_defined_names,
};
use super::output::{
    readback_xlsx_defined_name, xlsx_defined_name_item_json, xlsx_name_mutation_readback_commands,
};
use super::package::{select_xlsx_defined_name, xlsx_defined_names_with_workbook};
use super::workbook_xml::rewrite_workbook_defined_names;

pub(super) fn xlsx_names_mutate(
    file: &str,
    action: &str,
    options: XlsxNameMutationOptions<'_>,
) -> CliResult<Value> {
    if !Path::new(file).exists() {
        return Err(CliError::file_not_found(format!("file not found: {file}")));
    }
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;

    let (sheets, names, workbook_part, workbook_xml) = xlsx_defined_names_with_workbook(file)?;
    let mut updated_names = names.clone();
    let mut changed_name: Option<XlsxDefinedName> = None;
    let mut deleted_name: Option<XlsxDefinedName> = None;
    let mut previous_name = String::new();
    let mut previous_ref = String::new();

    match action {
        "add" => {
            let name = options
                .name
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| CliError::invalid_args("--name is required"))?;
            validate_defined_name(name).map_err(|message| {
                CliError::invalid_args(format!("failed to add defined name: {message}"))
            })?;
            let ref_text = resolve_defined_name_ref_from_flags(
                &sheets,
                options.ref_,
                options.sheet,
                options.range,
            )?;
            let local_sheet_id = resolve_defined_name_scope(&sheets, options.scope_sheet)?;
            if duplicate_defined_name(&updated_names, name, local_sheet_id, None) {
                return Err(CliError::invalid_args(format!(
                    "failed to add defined name: defined name {:?} already exists in {} scope",
                    name,
                    defined_name_scope_text(local_sheet_id)
                )));
            }
            let mut added = XlsxDefinedName {
                number: updated_names.len() as u32 + 1,
                name: name.to_string(),
                scope: defined_name_scope_text(local_sheet_id).to_string(),
                local_sheet_id,
                ref_text,
                hidden: options.hidden,
                comment: options.comment.unwrap_or("").trim().to_string(),
                ..XlsxDefinedName::default()
            };
            apply_defined_name_sheet_context(&mut added, &sheets);
            added.apply_selectors();
            updated_names.push(added.clone());
            changed_name = Some(added);
        }
        "update" => {
            let target = select_xlsx_defined_name(
                &sheets,
                &updated_names,
                options.name.unwrap_or(""),
                options.scope_sheet,
            )?;
            let ref_text = resolve_defined_name_ref_from_flags(
                &sheets,
                options.ref_,
                options.sheet,
                options.range,
            )?;
            let index = defined_name_index(&updated_names, &target)?;
            previous_ref = updated_names[index].ref_text.trim().to_string();
            check_expected_defined_name_ref(&previous_ref, options.expect_ref).map_err(
                |message| {
                    CliError::invalid_args(format!("failed to update defined name: {message}"))
                },
            )?;
            updated_names[index].ref_text = ref_text;
            updated_names[index].apply_selectors();
            changed_name = Some(updated_names[index].clone());
        }
        "rename" => {
            let target = select_xlsx_defined_name(
                &sheets,
                &updated_names,
                options.name.unwrap_or(""),
                options.scope_sheet,
            )?;
            let new_name = options.new_name.unwrap_or("").trim();
            validate_defined_name(new_name).map_err(|message| {
                CliError::invalid_args(format!("failed to rename defined name: {message}"))
            })?;
            let index = defined_name_index(&updated_names, &target)?;
            previous_ref = updated_names[index].ref_text.trim().to_string();
            check_expected_defined_name_ref(&previous_ref, options.expect_ref).map_err(
                |message| {
                    CliError::invalid_args(format!("failed to rename defined name: {message}"))
                },
            )?;
            let local_sheet_id = updated_names[index].local_sheet_id;
            if duplicate_defined_name(&updated_names, new_name, local_sheet_id, Some(index)) {
                return Err(CliError::invalid_args(format!(
                    "failed to rename defined name: defined name {:?} already exists in {} scope",
                    new_name,
                    defined_name_scope_text(local_sheet_id)
                )));
            }
            previous_name = updated_names[index].name.clone();
            updated_names[index].name = new_name.to_string();
            updated_names[index].apply_selectors();
            changed_name = Some(updated_names[index].clone());
        }
        "delete" => {
            let target = select_xlsx_defined_name(
                &sheets,
                &updated_names,
                options.name.unwrap_or(""),
                options.scope_sheet,
            )?;
            let index = defined_name_index(&updated_names, &target)?;
            previous_ref = updated_names[index].ref_text.trim().to_string();
            check_expected_defined_name_ref(&previous_ref, options.expect_ref).map_err(
                |message| {
                    CliError::invalid_args(format!("failed to delete defined name: {message}"))
                },
            )?;
            deleted_name = Some(updated_names.remove(index));
            renumber_defined_names(&mut updated_names);
        }
        _ => {
            return Err(CliError::unexpected(format!(
                "unknown name mutation action {action:?}"
            )));
        }
    }

    let updated_workbook_xml = rewrite_workbook_defined_names(&workbook_xml, &updated_names)?;
    let output_path = options.out.filter(|value| !value.trim().is_empty());
    let commit_path = if options.in_place {
        Some(file)
    } else {
        output_path
    };
    let readback_path = if options.dry_run || options.in_place || output_path == Some(file) {
        xlsx_ranges_set_temp_path(file)
    } else {
        output_path
            .ok_or_else(|| {
                CliError::invalid_args(
                    "must specify exactly one of --out, --in-place, or --dry-run",
                )
            })?
            .to_string()
    };
    let mut overrides = BTreeMap::new();
    overrides.insert(workbook_part, updated_workbook_xml);
    copy_zip_with_part_overrides(file, &readback_path, &overrides)?;
    if !options.no_validate {
        validate(&readback_path, true)?;
    }

    let readback_changed = if let Some(changed) = &changed_name {
        Some(readback_xlsx_defined_name(
            &readback_path,
            &changed.name,
            changed.local_sheet_id,
        )?)
    } else {
        None
    };
    if options.dry_run {
        let _ = fs::remove_file(&readback_path);
    } else if options.in_place || output_path == Some(file) {
        if let Some(backup_path) = options.backup.filter(|value| !value.trim().is_empty()) {
            fs::copy(file, backup_path)
                .map_err(|err| CliError::unexpected(format!("failed to create backup: {err}")))?;
        }
        fs::rename(&readback_path, file)
            .or_else(|_| {
                fs::copy(&readback_path, file)?;
                fs::remove_file(&readback_path)
            })
            .map_err(|err| CliError::unexpected(format!("failed to write output file: {err}")))?;
    }

    let readback_file = commit_path.unwrap_or("");
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    if let Some(commit_path) = commit_path {
        result.insert("output".to_string(), json!(commit_path));
    }
    result.insert("dryRun".to_string(), json!(options.dry_run));
    result.insert("action".to_string(), json!(action));
    if let Some(name) = &readback_changed {
        result.insert(
            "name".to_string(),
            xlsx_defined_name_item_json(readback_file, name, None),
        );
    }
    if let Some(name) = &deleted_name {
        result.insert(
            "deleted".to_string(),
            xlsx_defined_name_item_json(file, name, None),
        );
    }
    if !previous_name.is_empty() {
        result.insert("previousName".to_string(), json!(previous_name));
    }
    if !previous_ref.is_empty() && action == "update" {
        result.insert("previousRef".to_string(), json!(previous_ref));
    }
    let commands = xlsx_name_mutation_readback_commands(readback_file, readback_changed.as_ref());
    result.extend(commands);
    Ok(Value::Object(result))
}

fn resolve_defined_name_ref_from_flags(
    sheets: &[WorkbookSheet],
    exact_ref: Option<&str>,
    sheet_selector: Option<&str>,
    range_text: Option<&str>,
) -> CliResult<String> {
    let exact_ref = exact_ref.unwrap_or("").trim();
    let range_text = range_text.unwrap_or("").trim();
    if exact_ref.is_empty() == range_text.is_empty() {
        return Err(CliError::invalid_args(
            "must specify exactly one of --ref or --range",
        ));
    }
    if !exact_ref.is_empty() {
        return normalize_defined_name_ref(exact_ref).map_err(CliError::invalid_args);
    }
    let sheet_selector = sheet_selector.unwrap_or("").trim();
    if sheet_selector.is_empty() {
        return Err(CliError::invalid_args(
            "--sheet is required when using --range",
        ));
    }
    let sheet = resolve_sheet(sheets, sheet_selector)?;
    let range = parse_cli_range(range_text)?;
    Ok(format!(
        "{}!{}",
        quote_defined_name_sheet(&sheet.name),
        absolute_range_ref(range)
    ))
}

fn resolve_defined_name_scope(
    sheets: &[WorkbookSheet],
    scope_sheet: Option<&str>,
) -> CliResult<Option<i64>> {
    let Some(scope_sheet) = scope_sheet.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let sheet = resolve_sheet(sheets, scope_sheet)?;
    Ok(Some(sheet.position as i64 - 1))
}

fn validate_defined_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("defined name cannot be empty".to_string());
    }
    if name.chars().count() > 255 {
        return Err(format!(
            "defined name {:?} exceeds Excel's 255-character limit",
            name
        ));
    }
    let first = name.chars().next().unwrap_or_default();
    if !(first.is_alphabetic() || first == '_' || first == '\\') {
        return Err(format!(
            "defined name {:?} must start with a letter, underscore, or backslash",
            name
        ));
    }
    for ch in name.chars() {
        if ch.is_alphabetic() || ch.is_ascii_digit() || ch == '_' || ch == '.' || ch == '\\' {
            continue;
        }
        return Err(format!(
            "defined name {:?} contains invalid characters",
            name
        ));
    }
    if parse_cell_ref(name).is_ok() {
        return Err(format!(
            "defined name {:?} cannot be an A1 cell reference",
            name
        ));
    }
    if is_r1c1_name(name) {
        return Err(format!(
            "defined name {:?} cannot be an R1C1 cell reference",
            name
        ));
    }
    Ok(())
}

fn is_r1c1_name(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    let Some(rest) = upper.strip_prefix('R') else {
        return false;
    };
    let Some((row, col)) = rest.split_once('C') else {
        return false;
    };
    !row.is_empty()
        && !col.is_empty()
        && row.chars().all(|ch| ch.is_ascii_digit())
        && col.chars().all(|ch| ch.is_ascii_digit())
}

fn normalize_defined_name_ref(ref_text: &str) -> Result<String, String> {
    let normalized = ref_text
        .trim()
        .strip_prefix('=')
        .unwrap_or(ref_text.trim())
        .trim()
        .to_string();
    if normalized.is_empty() {
        Err("defined name ref cannot be empty".to_string())
    } else {
        Ok(normalized)
    }
}

fn quote_defined_name_sheet(sheet_name: &str) -> String {
    format!("'{}'", sheet_name.replace('\'', "''"))
}

fn absolute_range_ref(range: RangeBounds) -> String {
    let range = range.normalized();
    let start = format!("${}${}", col_name(range.start_col), range.start_row);
    let end = format!("${}${}", col_name(range.end_col), range.end_row);
    if start == end {
        start
    } else {
        format!("{start}:{end}")
    }
}

fn duplicate_defined_name(
    names: &[XlsxDefinedName],
    name: &str,
    local_sheet_id: Option<i64>,
    skip_index: Option<usize>,
) -> bool {
    names.iter().enumerate().any(|(index, candidate)| {
        skip_index != Some(index)
            && candidate.name.eq_ignore_ascii_case(name)
            && candidate.local_sheet_id == local_sheet_id
    })
}

fn defined_name_index(names: &[XlsxDefinedName], target: &XlsxDefinedName) -> CliResult<usize> {
    if target.number > 0 {
        let index = target.number as usize - 1;
        if let Some(candidate) = names.get(index)
            && candidate.name.eq_ignore_ascii_case(&target.name)
            && candidate.local_sheet_id == target.local_sheet_id
        {
            return Ok(index);
        }
    }
    names
        .iter()
        .position(|candidate| {
            candidate.name.eq_ignore_ascii_case(&target.name)
                && candidate.local_sheet_id == target.local_sheet_id
        })
        .ok_or_else(|| {
            CliError::unexpected(format!(
                "defined name {:?} is no longer present in {} scope",
                target.name, target.scope
            ))
        })
}

fn check_expected_defined_name_ref(actual: &str, expected: Option<&str>) -> Result<(), String> {
    let expected = expected
        .unwrap_or("")
        .trim()
        .strip_prefix('=')
        .unwrap_or(expected.unwrap_or("").trim())
        .trim();
    if expected.is_empty() {
        return Ok(());
    }
    if actual != expected {
        Err(format!(
            "defined name ref mismatch: expected {:?}, found {:?}",
            expected, actual
        ))
    } else {
        Ok(())
    }
}
