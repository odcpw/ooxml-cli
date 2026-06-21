mod authoring;
mod cfb;
mod create;
mod inspect;
mod model;
mod mutation;
mod office_check;
mod output;
mod package_xml;
mod source;

use serde_json::{Map, Value, json};
use std::fs;
use std::path::Path;

pub(crate) use authoring::{
    VbaBuildBinOptions, VbaPureCreateOptions, VbaRebuildOptions, vba_build_bin, vba_create_pure,
    vba_rebuild,
};
pub(crate) use create::{VbaCreateOptions, vba_create};
pub(crate) use model::VbaMutationOptions;
pub(crate) use office_check::vba_office_check;

use crate::{CliError, CliResult, zip_bytes};
use inspect::inspect_vba_package;
use mutation::{attach_vba_project, remove_vba_project};
use output::{
    vba_attach_template_for_bin, vba_extract_bin_command, vba_info_json, vba_inspect_command,
    vba_next_mutation_template, vba_office_check_command, vba_package_readback_command,
    vba_validate_command,
};
use package_xml::package_part_name;

pub(crate) use source::{
    VbaAddModuleOptions, VbaRemoveModuleOptions, VbaReplaceModuleOptions, vba_add_module,
    vba_extract, vba_inspect_bin, vba_list, vba_remove_module, vba_replace_module,
};

pub(crate) fn vba_inspect(file: &str) -> CliResult<Value> {
    let info = inspect_vba_package(file)?;
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("vba".to_string(), vba_info_json(&info));
    result.insert(
        "validateCommand".to_string(),
        json!(vba_validate_command(file)),
    );
    if let Some(command) = vba_office_check_command(file, &info) {
        result.insert("officeCheckCommand".to_string(), json!(command));
    }
    result.insert(
        "packageReadbackCommand".to_string(),
        json!(vba_package_readback_command(file, info.family.family)),
    );
    if let Some(command) = vba_extract_bin_command(file, &info) {
        result.insert("extractBinCommand".to_string(), json!(command));
    }
    result.insert(
        "nextMutationTemplate".to_string(),
        json!(vba_next_mutation_template(file, &info)),
    );
    Ok(Value::Object(result))
}

pub(crate) fn vba_extract_bin(file: &str, out: &str) -> CliResult<Value> {
    if out.trim().is_empty() {
        return Err(CliError::invalid_args("--out is required"));
    }
    let info = inspect_vba_package(file)?;
    let project = info
        .project
        .as_ref()
        .filter(|project| project.exists)
        .ok_or_else(|| CliError::target_not_found("package has no vbaProject.bin part"))?;
    let part_name = package_part_name(&project.part_uri);
    let data = zip_bytes(file, &part_name)?;
    if let Some(parent) = Path::new(out).parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|err| {
            CliError::unexpected(format!("failed to create output directory: {err}"))
        })?;
    }
    fs::write(out, &data)
        .map_err(|err| CliError::unexpected(format!("failed to write VBA binary: {err}")))?;

    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("output".to_string(), json!(out));
    result.insert("bytesWritten".to_string(), json!(data.len()));
    result.insert("vba".to_string(), vba_info_json(&info));
    result.insert(
        "inspectCommand".to_string(),
        json!(vba_inspect_command(file)),
    );
    result.insert(
        "validateCommand".to_string(),
        json!(vba_validate_command(file)),
    );
    if let Some(command) = vba_office_check_command(file, &info) {
        result.insert("officeCheckCommand".to_string(), json!(command));
    }
    result.insert(
        "packageReadbackCommand".to_string(),
        json!(vba_package_readback_command(file, info.family.family)),
    );
    result.insert(
        "attachCommandTemplate".to_string(),
        json!(vba_attach_template_for_bin(out, &info)),
    );
    Ok(Value::Object(result))
}

pub(crate) fn vba_attach(
    file: &str,
    bin_path: &str,
    options: VbaMutationOptions<'_>,
) -> CliResult<Value> {
    attach_vba_project(file, bin_path, options)
}

pub(crate) fn vba_remove(file: &str, options: VbaMutationOptions<'_>) -> CliResult<Value> {
    remove_vba_project(file, options)
}
