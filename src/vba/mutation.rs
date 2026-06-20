use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use crate::{
    CliError, CliResult, copy_zip_with_binary_part_overrides_and_removals,
    package_mutation_temp_path, relationships_part_for, validate,
    validate_xlsx_mutation_output_flags, zip_bytes, zip_text,
};

use super::inspect::{candidate_vba_project_parts, inspect_vba_package};
use super::model::{VBA_PROJECT_CONTENT_TYPE, VbaInfo, VbaMutationOptions};
use super::output::{
    vba_extract_bin_command, vba_info_json, vba_inspect_command, vba_mutation_result_json,
    vba_next_mutation_template, vba_office_check_command, vba_output_placeholder,
    vba_package_readback_command, vba_validate_command,
};
use super::package_xml::{
    package_part_name, remove_content_type_override, remove_vba_relationships_xml,
    set_content_type_override, upsert_vba_relationship_xml,
};

pub(super) fn attach_vba_project(
    file: &str,
    bin_path: &str,
    options: VbaMutationOptions<'_>,
) -> CliResult<Value> {
    if bin_path.trim().is_empty() {
        return Err(CliError::invalid_args("--bin is required"));
    }
    let data = fs::read(bin_path)
        .map_err(|err| CliError::file_not_found(format!("failed to read VBA binary: {err}")))?;
    if data.is_empty() {
        return Err(CliError::invalid_args("vbaProject.bin is empty"));
    }
    let info = inspect_vba_package(file)?;
    let target_part = info
        .project
        .as_ref()
        .map(|project| project.part_uri.clone())
        .filter(|part| !part.trim().is_empty())
        .unwrap_or_else(|| info.family.default_vba_part.to_string());
    let main_part = package_part_name(&info.main_part_uri);
    let main_data = zip_bytes(file, &main_part)?;
    let mut text_overrides = BTreeMap::new();
    let mut binary_overrides = BTreeMap::new();
    let mut removals = BTreeSet::new();
    let content_types = set_content_type_override(
        &set_content_type_override(
            &zip_text(file, "[Content_Types].xml")?,
            &info.main_part_uri,
            info.family.macro_content_type,
        ),
        &target_part,
        VBA_PROJECT_CONTENT_TYPE,
    );
    let rels_part = relationships_part_for(&info.main_part_uri);
    let rels_xml = zip_text(file, &rels_part).unwrap_or_else(|_| {
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"></Relationships>"#.to_string()
    });
    text_overrides.insert("[Content_Types].xml".to_string(), content_types);
    text_overrides.insert(
        rels_part,
        upsert_vba_relationship_xml(&rels_xml, file, &info, &target_part),
    );
    binary_overrides.insert(main_part, main_data);
    binary_overrides.insert(package_part_name(&target_part), data);

    write_vba_mutation(
        file,
        options,
        &text_overrides,
        &binary_overrides,
        &mut removals,
        inspect_vba_package,
        |output_info| vba_mutation_result_json("attach", output_info, Some(&target_part), true),
    )
}

pub(super) fn remove_vba_project(file: &str, options: VbaMutationOptions<'_>) -> CliResult<Value> {
    let info = inspect_vba_package(file)?;
    let main_part = package_part_name(&info.main_part_uri);
    let main_data = zip_bytes(file, &main_part)?;
    let mut text_overrides = BTreeMap::new();
    let mut binary_overrides = BTreeMap::new();
    let candidate_parts = candidate_vba_project_parts(file, &info)?;
    let mut removals = candidate_parts
        .into_iter()
        .flat_map(|part| {
            let mut parts = vec![package_part_name(&part)];
            parts.push(relationships_part_for(&part));
            parts
        })
        .collect::<BTreeSet<_>>();
    let removed_part = removals
        .iter()
        .find(|part| part.ends_with("vbaProject.bin"))
        .map(|part| format!("/{part}"));
    let mut content_types = set_content_type_override(
        &zip_text(file, "[Content_Types].xml")?,
        &info.main_part_uri,
        info.family.non_macro_content_type,
    );
    for part in &removals {
        if part.ends_with("vbaProject.bin") {
            content_types = remove_content_type_override(&content_types, part);
        }
    }
    let rels_part = relationships_part_for(&info.main_part_uri);
    let rels_xml = zip_text(file, &rels_part).unwrap_or_else(|_| {
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"></Relationships>"#.to_string()
    });
    text_overrides.insert("[Content_Types].xml".to_string(), content_types);
    text_overrides.insert(
        rels_part,
        remove_vba_relationships_xml(&rels_xml, file, &info),
    );
    binary_overrides.insert(main_part, main_data);

    write_vba_mutation(
        file,
        options,
        &text_overrides,
        &binary_overrides,
        &mut removals,
        inspect_vba_package,
        |output_info| {
            vba_mutation_result_json(
                "remove",
                output_info,
                removed_part.as_deref().or_else(|| {
                    info.project
                        .as_ref()
                        .map(|project| project.part_uri.as_str())
                }),
                false,
            )
        },
    )
}

fn write_vba_mutation<F, G>(
    file: &str,
    options: VbaMutationOptions<'_>,
    text_overrides: &BTreeMap<String, String>,
    binary_overrides: &BTreeMap<String, Vec<u8>>,
    removals: &mut BTreeSet<String>,
    readback: F,
    mutation_result: G,
) -> CliResult<Value>
where
    F: Fn(&str) -> CliResult<VbaInfo>,
    G: Fn(&VbaInfo) -> Value,
{
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;
    let output_path = options.out.filter(|value| !value.trim().is_empty());
    let commit_path = if options.in_place {
        Some(file)
    } else {
        output_path
    };
    let readback_path = if options.dry_run || options.in_place || output_path == Some(file) {
        package_mutation_temp_path(file, "vba-mutation")
    } else {
        output_path
            .ok_or_else(|| {
                CliError::invalid_args(
                    "must specify exactly one of --out, --in-place, or --dry-run",
                )
            })?
            .to_string()
    };

    for key in text_overrides.keys().chain(binary_overrides.keys()) {
        removals.remove(key);
    }
    copy_zip_with_binary_part_overrides_and_removals(
        file,
        &readback_path,
        text_overrides,
        binary_overrides,
        removals,
    )?;
    if !options.no_validate {
        validate(&readback_path, true)?;
    }
    let output_info = readback(&readback_path)?;

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

    let target = if options.dry_run {
        vba_output_placeholder(if output_info.macro_enabled {
            output_info.family.macro_extension
        } else {
            output_info.family.non_macro_extension
        })
    } else {
        commit_path.unwrap_or(&readback_path).to_string()
    };
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    if !options.dry_run
        && let Some(commit_path) = commit_path
    {
        result.insert("output".to_string(), json!(commit_path));
    }
    if options.dry_run {
        result.insert("dryRun".to_string(), json!(true));
    }
    result.insert("result".to_string(), mutation_result(&output_info));
    result.insert("vba".to_string(), vba_info_json(&output_info));
    if options.dry_run {
        result.insert(
            "inspectCommandTemplate".to_string(),
            json!(vba_inspect_command(&target)),
        );
        result.insert(
            "validateCommandTemplate".to_string(),
            json!(vba_validate_command(&target)),
        );
        if let Some(command) = vba_office_check_command(&target, &output_info) {
            result.insert("officeCheckCommandTemplate".to_string(), json!(command));
        }
        result.insert(
            "packageReadbackCommandTemplate".to_string(),
            json!(vba_package_readback_command(
                &target,
                output_info.family.family
            )),
        );
    } else {
        result.insert(
            "inspectCommand".to_string(),
            json!(vba_inspect_command(&target)),
        );
        result.insert(
            "validateCommand".to_string(),
            json!(vba_validate_command(&target)),
        );
        if let Some(command) = vba_office_check_command(&target, &output_info) {
            result.insert("officeCheckCommand".to_string(), json!(command));
        }
        result.insert(
            "packageReadbackCommand".to_string(),
            json!(vba_package_readback_command(
                &target,
                output_info.family.family
            )),
        );
        if let Some(command) = vba_extract_bin_command(&target, &output_info) {
            result.insert("extractBinCommand".to_string(), json!(command));
        }
        result.insert(
            "nextMutationTemplate".to_string(),
            json!(vba_next_mutation_template(&target, &output_info)),
        );
    }
    Ok(Value::Object(result))
}
