mod cfb_paths;
mod codec;
mod compatibility;
mod dir;
mod mutation;
mod output_json;
mod parser;
mod project_metadata;
mod selectors;

use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::{
    CliError, CliResult, copy_zip_with_binary_part_overrides_and_removals,
    package_mutation_temp_path, validate, validate_xlsx_mutation_output_flags, zip_bytes,
};

use super::inspect::inspect_vba_package;
use super::model::{VbaInfo, VbaMutationOptions};
use super::output::{
    vba_conformance_command, vba_extract_modules_template, vba_info_json, vba_inspect_command,
    vba_list_command, vba_office_check_command, vba_output_placeholder,
    vba_package_readback_command, vba_standalone_attach_template, vba_validate_command,
};
use super::package_xml::package_part_name;

use mutation::{
    AddModuleOptions, SourceMutationOptions, SourceMutationResult,
    add_module_source_in_project_data, remove_module_source_in_project_data,
    replace_module_source_in_project_data,
};
use output_json::{module_extract_item_json, source_project_json, vba_source_mutation_output_json};
use parser::{
    inspect_source_project_for_file, normalize_inspect_bin_family, parse_source_project_for_family,
};
use selectors::{module_output_name, select_modules};

const DIR_STREAM_PATH: &str = "VBA/dir";
#[derive(Clone)]
pub(super) struct SourceProject {
    family: String,
    part_uri: String,
    code_page: i32,
    module_count: usize,
    modules: Vec<SourceModule>,
    project_metadata: Option<ProjectMetadata>,
    office_compatibility: OfficeCompatibilityReport,
    host_compatibility_warnings: Vec<HostCompatibilityWarning>,
    warnings: Vec<String>,
}

#[derive(Clone)]
pub(super) struct SourceModule {
    number: usize,
    name: String,
    stream_name: String,
    kind: String,
    extension: String,
    code_page: i32,
    source_offset: u32,
    source_bytes: Option<usize>,
    line_count: Option<usize>,
    sha256: String,
    sha256_basis: String,
    line_ending: String,
    trailing_newline: bool,
    primary_selector: String,
    selectors: Vec<String>,
    source: String,
    warnings: Vec<String>,
}

#[derive(Clone)]
struct ProjectMetadata {
    stream_name: String,
    present: bool,
    line_count: usize,
    id: String,
    name: String,
    modules: Vec<ProjectModuleDeclaration>,
    references: Vec<ProjectReference>,
    workspace_entries: Vec<ProjectWorkspaceEntry>,
    has_project_wm: bool,
    project_wm_stream: String,
    warnings: Vec<String>,
}

#[derive(Clone)]
struct ProjectModuleDeclaration {
    kind: String,
    name: String,
    value: String,
    line: usize,
}

#[derive(Clone)]
struct ProjectReference {
    kind: String,
    value: String,
    line: usize,
}

#[derive(Clone)]
struct ProjectWorkspaceEntry {
    name: String,
    value: String,
    line: usize,
}

#[derive(Clone)]
struct OfficeCompatibilityReport {
    office_load_verified: bool,
    status: String,
    risks: Vec<HostCompatibilityWarning>,
    notes: Vec<String>,
}

#[derive(Clone)]
struct HostCompatibilityWarning {
    code: String,
    message: String,
    modules: Vec<String>,
}

pub(crate) struct VbaAddModuleOptions<'a> {
    pub(crate) source: &'a str,
    pub(crate) name: Option<&'a str>,
    pub(crate) kind: Option<&'a str>,
    pub(crate) expect_module_count: Option<usize>,
    pub(crate) allow_experimental_vba_source_rewrite: bool,
    pub(crate) mutation: VbaMutationOptions<'a>,
}

pub(crate) struct VbaReplaceModuleOptions<'a> {
    pub(crate) module: &'a str,
    pub(crate) source: &'a str,
    pub(crate) expect_sha256: Option<&'a str>,
    pub(crate) allow_experimental_vba_source_rewrite: bool,
    pub(crate) mutation: VbaMutationOptions<'a>,
}

pub(crate) struct VbaRemoveModuleOptions<'a> {
    pub(crate) module: &'a str,
    pub(crate) expect_sha256: Option<&'a str>,
    pub(crate) allow_experimental_vba_source_rewrite: bool,
    pub(crate) mutation: VbaMutationOptions<'a>,
}

pub(crate) fn vba_inspect_bin(bin_path: &str, family: &str) -> CliResult<Value> {
    let family = normalize_inspect_bin_family(family)?;
    let data = fs::read(bin_path)
        .map_err(|err| CliError::file_not_found(format!("failed to read VBA binary: {err}")))?;
    let project = parse_source_project_for_family(&data, &family)?;
    let mut hasher = Sha256::new();
    hasher.update(&data);

    let mut result = Map::new();
    result.insert("file".to_string(), json!(bin_path));
    result.insert("sizeBytes".to_string(), json!(data.len()));
    result.insert(
        "sha256".to_string(),
        json!(format!("{:x}", hasher.finalize())),
    );
    result.insert("family".to_string(), json!(project.family));
    result.insert("project".to_string(), source_project_json(&project, false));
    result.insert(
        "attachCommandTemplate".to_string(),
        json!(vba_standalone_attach_template(bin_path, &project.family)),
    );
    Ok(Value::Object(result))
}

pub(crate) fn vba_list(file: &str) -> CliResult<Value> {
    let (info, project) = inspect_source_project_for_file(file)?;
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("vba".to_string(), vba_info_json(&info));
    result.insert("project".to_string(), source_project_json(&project, false));
    result.insert(
        "inspectCommand".to_string(),
        json!(vba_inspect_command(file)),
    );
    result.insert(
        "validateCommand".to_string(),
        json!(vba_validate_command(file)),
    );
    result.insert(
        "conformanceCommand".to_string(),
        json!(vba_conformance_command(file)),
    );
    if let Some(command) = vba_office_check_command(file, &info) {
        result.insert("officeCheckCommand".to_string(), json!(command));
    }
    result.insert(
        "packageReadbackCommand".to_string(),
        json!(vba_package_readback_command(file, info.family.family)),
    );
    result.insert(
        "extractCommandTemplate".to_string(),
        json!(vba_extract_modules_template(file)),
    );
    Ok(Value::Object(result))
}

pub(crate) fn vba_extract(file: &str, out_dir: &str, selector: Option<&str>) -> CliResult<Value> {
    if out_dir.trim().is_empty() {
        return Err(CliError::invalid_args("--out-dir is required"));
    }
    let (info, project) = inspect_source_project_for_file(file)?;
    let modules = select_modules(file, &project.modules, selector.unwrap_or_default())?;
    if modules.is_empty() {
        return Err(CliError::target_not_found("no VBA modules to extract"));
    }
    fs::create_dir_all(out_dir).map_err(|err| {
        CliError::unexpected(format!("failed to create module output directory: {err}"))
    })?;

    let mut used = BTreeMap::<String, usize>::new();
    let mut extracted = Vec::new();
    for module in &modules {
        let mut name = module_output_name(module);
        let count = used.entry(name.clone()).or_insert(0);
        if *count > 0 {
            let path = Path::new(&name);
            let extension = path
                .extension()
                .and_then(|value| value.to_str())
                .map(|value| format!(".{value}"))
                .unwrap_or_default();
            let base = if extension.is_empty() {
                name.clone()
            } else {
                name.trim_end_matches(&extension).to_string()
            };
            name = format!("{base}-{}{}", *count + 1, extension);
        }
        *used.entry(name.clone()).or_insert(0) += 1;
        let output_path = Path::new(out_dir).join(&name);
        fs::write(&output_path, module.source.as_bytes()).map_err(|err| {
            CliError::unexpected(format!("failed to write VBA module {}: {err}", module.name))
        })?;
        extracted.push(module_extract_item_json(
            module,
            &output_path,
            module.source.len(),
        ));
    }

    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("outputDir".to_string(), json!(out_dir));
    result.insert("vba".to_string(), vba_info_json(&info));
    result.insert("project".to_string(), source_project_json(&project, false));
    result.insert("modules".to_string(), Value::Array(extracted));
    result.insert(
        "inspectCommand".to_string(),
        json!(vba_inspect_command(file)),
    );
    result.insert(
        "validateCommand".to_string(),
        json!(vba_validate_command(file)),
    );
    result.insert(
        "conformanceCommand".to_string(),
        json!(vba_conformance_command(file)),
    );
    if let Some(command) = vba_office_check_command(file, &info) {
        result.insert("officeCheckCommand".to_string(), json!(command));
    }
    result.insert(
        "packageReadbackCommand".to_string(),
        json!(vba_package_readback_command(file, info.family.family)),
    );
    result.insert("listCommand".to_string(), json!(vba_list_command(file)));
    Ok(Value::Object(result))
}

pub(crate) fn vba_add_module(file: &str, options: VbaAddModuleOptions<'_>) -> CliResult<Value> {
    if options.source.trim().is_empty() {
        return Err(CliError::invalid_args("--source is required"));
    }
    let source = read_module_source_file(options.source)?;
    let mut name = options.name.unwrap_or_default().trim().to_string();
    if name.is_empty() {
        name = Path::new(options.source)
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_string();
    }
    let mut kind = options.kind.unwrap_or_default().trim().to_string();
    if kind.is_empty() {
        kind = Path::new(options.source)
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
    }
    write_vba_source_mutation(file, "add", options.mutation, |data, info| {
        let (mut result, rewritten) = add_module_source_in_project_data(
            data,
            &source,
            AddModuleOptions {
                name,
                kind,
                expect_module_count: options.expect_module_count,
                allow_experimental_source_rewrite: options.allow_experimental_vba_source_rewrite,
            },
        )?;
        result.family = info.family.family.to_string();
        if let Some(project) = &info.project {
            result.part_uri = project.part_uri.clone();
        }
        Ok((result, rewritten))
    })
}

pub(crate) fn vba_replace_module(
    file: &str,
    options: VbaReplaceModuleOptions<'_>,
) -> CliResult<Value> {
    if options.module.trim().is_empty() {
        return Err(CliError::invalid_args("--module is required"));
    }
    if options.source.trim().is_empty() {
        return Err(CliError::invalid_args("--source is required"));
    }
    let source = read_module_source_file(options.source)?;
    let source_kind = Path::new(options.source)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    write_vba_source_mutation(file, "replace", options.mutation, |data, info| {
        let (mut result, rewritten) = replace_module_source_in_project_data(
            file,
            data,
            options.module,
            &source,
            options.expect_sha256.unwrap_or_default(),
            SourceMutationOptions {
                allow_experimental_source_rewrite: options.allow_experimental_vba_source_rewrite,
                source_kind: source_kind.clone(),
            },
        )?;
        result.family = info.family.family.to_string();
        if let Some(project) = &info.project {
            result.part_uri = project.part_uri.clone();
        }
        Ok((result, rewritten))
    })
}

pub(crate) fn vba_remove_module(
    file: &str,
    options: VbaRemoveModuleOptions<'_>,
) -> CliResult<Value> {
    if options.module.trim().is_empty() {
        return Err(CliError::invalid_args("--module is required"));
    }
    write_vba_source_mutation(file, "remove", options.mutation, |data, info| {
        let (mut result, rewritten) = remove_module_source_in_project_data(
            file,
            data,
            options.module,
            options.expect_sha256.unwrap_or_default(),
            SourceMutationOptions {
                allow_experimental_source_rewrite: options.allow_experimental_vba_source_rewrite,
                source_kind: String::new(),
            },
        )?;
        result.family = info.family.family.to_string();
        if let Some(project) = &info.project {
            result.part_uri = project.part_uri.clone();
        }
        Ok((result, rewritten))
    })
}

fn write_vba_source_mutation<F>(
    file: &str,
    action_label: &str,
    options: VbaMutationOptions<'_>,
    mutate: F,
) -> CliResult<Value>
where
    F: FnOnce(&[u8], &VbaInfo) -> CliResult<(SourceMutationResult, Vec<u8>)>,
{
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;
    let info = inspect_vba_package(file)?;
    if !info.signature_artifacts.is_empty() {
        return Err(CliError::unexpected(format!(
            "refusing to {action_label} VBA module because known signature artifacts are present"
        )));
    }
    let project = info
        .project
        .as_ref()
        .filter(|project| project.exists)
        .ok_or_else(|| CliError::target_not_found("package has no vbaProject.bin part"))?;
    let part_name = package_part_name(&project.part_uri);
    let data = zip_bytes(file, &part_name)?;
    let (mutation_result, rewritten) = mutate(&data, &info)?;

    let output_path = options.out.filter(|value| !value.trim().is_empty());
    let commit_path = if options.in_place {
        Some(file)
    } else {
        output_path
    };
    let readback_path = if options.dry_run || options.in_place || output_path == Some(file) {
        package_mutation_temp_path(file, "vba-source-mutation")
    } else {
        output_path
            .ok_or_else(|| {
                CliError::invalid_args(
                    "must specify exactly one of --out, --in-place, or --dry-run",
                )
            })?
            .to_string()
    };

    let mut binary_overrides = BTreeMap::new();
    binary_overrides.insert(part_name, rewritten);
    let removals = BTreeSet::new();
    copy_zip_with_binary_part_overrides_and_removals(
        file,
        &readback_path,
        &BTreeMap::new(),
        &binary_overrides,
        &removals,
    )?;
    if !options.no_validate {
        validate(&readback_path, true)?;
    }

    let (mut output_info, mut output_project) = inspect_source_project_for_file(&readback_path)?;
    if !options.dry_run {
        if options.in_place || output_path == Some(file) {
            if let Some(backup_path) = options.backup.filter(|value| !value.trim().is_empty()) {
                fs::copy(file, backup_path).map_err(|err| {
                    CliError::unexpected(format!("failed to create backup: {err}"))
                })?;
            }
            fs::rename(&readback_path, file)
                .or_else(|_| {
                    fs::copy(&readback_path, file)?;
                    fs::remove_file(&readback_path)
                })
                .map_err(|err| {
                    CliError::unexpected(format!("failed to write output file: {err}"))
                })?;
            (output_info, output_project) = inspect_source_project_for_file(file)?;
        } else if let Some(path) = output_path {
            (output_info, output_project) = inspect_source_project_for_file(path)?;
        }
    } else {
        let _ = fs::remove_file(&readback_path);
    }

    let target = if options.dry_run {
        vba_output_placeholder(output_info.family.macro_extension)
    } else {
        commit_path.unwrap_or(&readback_path).to_string()
    };
    Ok(vba_source_mutation_output_json(
        file,
        if options.dry_run { None } else { commit_path },
        options.dry_run,
        &mutation_result,
        &output_info,
        &output_project,
        &target,
    ))
}

fn read_module_source_file(path: &str) -> CliResult<Vec<u8>> {
    if path.trim().is_empty() {
        return Err(CliError::invalid_args("source path is required"));
    }
    let data = fs::read(path).map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            let mut message = err.to_string();
            if let Some((before, _)) = message.split_once(" (os error ") {
                message = before.to_string();
            }
            return CliError::file_not_found(format!(
                "failed to read VBA source: open {path}: {message}"
            ));
        }
        CliError::invalid_args(format!("failed to read VBA source: {err}"))
    })?;
    if data.is_empty() {
        return Err(CliError::invalid_args(
            "failed to read VBA source: VBA source file is empty",
        ));
    }
    Ok(data)
}
