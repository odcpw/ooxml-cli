use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::{
    CliError, CliResult, command_arg, copy_zip_with_binary_part_overrides_and_removals,
    package_mutation_temp_path, selector_candidates, validate, validate_xlsx_mutation_output_flags,
    zip_bytes,
};

use super::cfb::{CfbFile, rewrite_streams_with_adds_and_deletes, rewrite_streams_with_deletes};
use super::inspect::inspect_vba_package;
use super::model::{VbaInfo, VbaMutationOptions};
use super::output::{
    vba_extract_modules_template, vba_info_json, vba_inspect_command, vba_list_command,
    vba_next_mutation_template, vba_office_check_command, vba_output_placeholder,
    vba_package_readback_command, vba_standalone_attach_template, vba_validate_command,
};
use super::package_xml::package_part_name;

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

#[derive(Clone, Default)]
struct DirModule {
    name: String,
    stream_name: String,
    kind: String,
    source_offset: u32,
}

struct ProjectModulesRecord {
    record_start: usize,
    count_payload: usize,
    count: usize,
    modules_start: usize,
    modules_end: usize,
}

struct DirReader<'a> {
    data: &'a [u8],
    pos: usize,
    code_page: i32,
    modules: Vec<DirModule>,
    warnings: Vec<String>,
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

struct SourceMutationOptions {
    allow_experimental_source_rewrite: bool,
    source_kind: String,
}

struct AddModuleOptions {
    name: String,
    kind: String,
    expect_module_count: Option<usize>,
    allow_experimental_source_rewrite: bool,
}

struct SourceMutationResult {
    action: String,
    family: String,
    part_uri: String,
    module: SourceModule,
    previous_count: Option<usize>,
    module_count: Option<usize>,
    previous_sha256: String,
    sha256: String,
    source_bytes: Option<usize>,
    line_count: Option<usize>,
    warnings: Vec<String>,
    purged_caches: bool,
    recompiles_on_open: bool,
    office_load_verified: bool,
    compatibility_status: String,
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

fn replace_module_source_in_project_data(
    file: &str,
    data: &[u8],
    selector: &str,
    source: &[u8],
    expect_sha256: &str,
    options: SourceMutationOptions,
) -> CliResult<(SourceMutationResult, Vec<u8>)> {
    let project = parse_source_project(data).map_err(map_source_parse_error)?;
    let module = select_one_module(file, &project.modules, selector)?;
    let expected = normalize_sha256_guard(expect_sha256);
    if !expected.is_empty() && !expected.eq_ignore_ascii_case(&module.sha256) {
        return Err(CliError::invalid_args(format!(
            "VBA module source hash mismatch: expected {expected} but found {}",
            module.sha256
        )));
    }
    if module.stream_name.is_empty() {
        return Err(CliError::unexpected(format!(
            "VBA module {:?} has no stream name",
            module.name
        )));
    }
    validate_replacement_module_source(&module, source, &options.source_kind)?;
    let (encoded_source, mut warnings) = encode_module_source(source, module.code_page)?;
    let normalized_hash = source_sha256(&encoded_source, module.code_page);
    if normalized_hash == module.sha256 {
        let mut unchanged = module.clone();
        unchanged.source.clear();
        warnings.push(
            "replacement source is unchanged; preserved original vbaProject.bin bytes".to_string(),
        );
        return Ok((
            SourceMutationResult {
                action: "replace-module".to_string(),
                family: String::new(),
                part_uri: String::new(),
                module: unchanged,
                previous_count: None,
                module_count: None,
                previous_sha256: module.sha256.clone(),
                sha256: module.sha256.clone(),
                source_bytes: module.source_bytes,
                line_count: module.line_count,
                warnings,
                purged_caches: false,
                recompiles_on_open: false,
                office_load_verified: false,
                compatibility_status: "unchanged".to_string(),
            },
            data.to_vec(),
        ));
    }

    let cfb_file = CfbFile::open(data).map_err(map_source_parse_error)?;
    require_experimental_source_rewrite_allowed(
        &project,
        &cfb_file,
        options.allow_experimental_source_rewrite,
    )?;
    let dir_compressed = cfb_file
        .stream(DIR_STREAM_PATH)
        .map_err(CliError::unexpected)?;
    let dir_data = decompress_container(&dir_compressed).map_err(|err| {
        CliError::unexpected(format!("failed to decompress VBA dir stream: {err}"))
    })?;
    let (patched_dir, patched_offsets) = rewrite_dir_module_offset(&dir_data, &module, 0)?;
    if patched_offsets != 1 {
        return Err(CliError::unexpected(format!(
            "VBA dir stream patched {patched_offsets} MODULEOFFSET records for {}, want 1",
            module.name
        )));
    }
    let stream_path = format!("VBA/{}", module.stream_name);
    let module_stream_data = cfb_file
        .stream(&stream_path)
        .map_err(CliError::unexpected)?;
    if module.source_offset as usize > module_stream_data.len() {
        return Err(CliError::unexpected(format!(
            "source offset {} exceeds module stream {stream_path} size {}",
            module.source_offset,
            module_stream_data.len()
        )));
    }

    let mut replacements = BTreeMap::new();
    replacements.insert(
        DIR_STREAM_PATH.to_string(),
        compress_container_literals(&patched_dir),
    );
    replacements.insert(stream_path, compress_container_literals(&encoded_source));
    let delete_streams = vba_compiled_cache_streams(&cfb_file.streams());
    if !delete_streams.is_empty() {
        warnings.push(format!(
            "removed {} VBA compiled cache stream(s)",
            delete_streams.len()
        ));
    }
    if module.source_offset > 0 {
        warnings.push(format!(
            "removed performance-cache prefix from edited module {}; untouched module streams were preserved",
            module.name
        ));
    }
    warnings.push(
        "rewrote edited module source at MODULEOFFSET 0; Office compatibility remains unverified"
            .to_string(),
    );
    let rewritten = rewrite_streams_with_deletes(data, &replacements, &delete_streams)
        .map_err(CliError::unexpected)?;
    let updated_project = parse_source_project(&rewritten).map_err(map_source_parse_error)?;
    let mut updated_module =
        select_one_module(file, &updated_project.modules, &module.primary_selector)?;
    updated_module.source.clear();

    Ok((
        SourceMutationResult {
            action: "replace-module".to_string(),
            family: String::new(),
            part_uri: String::new(),
            module: updated_module.clone(),
            previous_count: None,
            module_count: None,
            previous_sha256: module.sha256,
            sha256: updated_module.sha256.clone(),
            source_bytes: updated_module.source_bytes,
            line_count: updated_module.line_count,
            warnings,
            purged_caches: true,
            recompiles_on_open: true,
            office_load_verified: false,
            compatibility_status: "experimental".to_string(),
        },
        rewritten,
    ))
}

fn add_module_source_in_project_data(
    data: &[u8],
    source: &[u8],
    options: AddModuleOptions,
) -> CliResult<(SourceMutationResult, Vec<u8>)> {
    let project = parse_source_project(data).map_err(map_source_parse_error)?;
    if let Some(expected) = options.expect_module_count
        && expected != project.modules.len()
    {
        return Err(CliError::invalid_args(format!(
            "VBA module count mismatch: expected {expected} but found {}",
            project.modules.len()
        )));
    }
    let name = resolve_added_module_name(source, &options.name)?;
    let kind = normalize_added_module_kind(&options.kind)?;
    validate_added_module_name(&name)?;
    for existing in &project.modules {
        if existing.name.eq_ignore_ascii_case(&name)
            || existing.stream_name.eq_ignore_ascii_case(&name)
        {
            return Err(CliError::invalid_args(format!(
                "VBA module {name:?} already exists"
            )));
        }
    }

    let cfb_file = CfbFile::open(data).map_err(map_source_parse_error)?;
    require_experimental_source_rewrite_allowed(
        &project,
        &cfb_file,
        options.allow_experimental_source_rewrite,
    )?;
    if has_version_dependent_project_metadata(&cfb_file) {
        return Err(CliError::unexpected(
            "refusing to add VBA module because this Office-shaped project has version-dependent _VBA_PROJECT metadata that must be regenerated for module-set changes; create an Office-authored vbaProject.bin seed and attach it, or replace an existing module",
        ));
    }
    if cfb_file.stream(&format!("VBA/{name}")).is_ok() {
        return Err(CliError::invalid_args(format!(
            "VBA module stream {name:?} already exists"
        )));
    }

    let dir_compressed = cfb_file
        .stream(DIR_STREAM_PATH)
        .map_err(CliError::unexpected)?;
    let dir_data = decompress_container(&dir_compressed).map_err(|err| {
        CliError::unexpected(format!("failed to decompress VBA dir stream: {err}"))
    })?;
    let mut added_module = SourceModule {
        number: project.modules.len() + 1,
        name: name.clone(),
        stream_name: name.clone(),
        kind: kind.clone(),
        extension: extension_for_module_kind(&kind).to_string(),
        code_page: project.code_page,
        source_offset: 0,
        source_bytes: None,
        line_count: None,
        sha256: String::new(),
        sha256_basis: String::new(),
        line_ending: String::new(),
        trailing_newline: false,
        primary_selector: format!("module:{name}"),
        selectors: Vec::new(),
        source: String::new(),
        warnings: Vec::new(),
    };
    added_module = with_source_module_selectors(added_module);
    let added_dir = add_dir_module(&dir_data, &added_module)?;
    let (encoded_source, mut warnings) =
        prepare_added_module_source(source, &name, project.code_page)?;

    let streams = cfb_file.streams();
    let mut replacements = BTreeMap::new();
    replacements.insert(
        DIR_STREAM_PATH.to_string(),
        compress_container_literals(&added_dir),
    );
    if let Some(project_path) = optional_cfb_stream_path(&streams, "PROJECT") {
        let project_data = cfb_file
            .stream(&project_path)
            .map_err(CliError::unexpected)?;
        let (patched_project, added_lines) =
            add_project_stream_module_lines(&project_data, &added_module)?;
        replacements.insert(project_path, patched_project);
        warnings.push(format!(
            "added {added_lines} PROJECT stream line(s) for module {name}"
        ));
    } else {
        warnings
            .push("PROJECT stream was not present; skipped project metadata update".to_string());
    }
    if let Some(project_wm_path) = optional_cfb_stream_path(&streams, "PROJECTwm") {
        let project_wm_data = cfb_file
            .stream(&project_wm_path)
            .map_err(CliError::unexpected)?;
        let (patched_project_wm, added_entries) =
            add_project_wm_module_entry(&project_wm_data, &added_module)?;
        replacements.insert(project_wm_path, patched_project_wm);
        warnings.push(format!(
            "added {added_entries} PROJECTwm entry(s) for module {name}"
        ));
    }

    let mut additions = BTreeMap::new();
    additions.insert(
        format!("VBA/{name}"),
        compress_container_literals(&encoded_source),
    );
    let delete_streams = vba_compiled_cache_streams(&streams);
    if !delete_streams.is_empty() {
        warnings.push(format!(
            "removed {} VBA compiled cache stream(s)",
            delete_streams.len()
        ));
    }
    warnings.push("added module source at MODULEOFFSET 0; untouched module streams were preserved and Office compatibility remains unverified".to_string());
    let rewritten =
        rewrite_streams_with_adds_and_deletes(data, &replacements, &additions, &delete_streams)
            .map_err(CliError::unexpected)?;
    let updated_project = parse_source_project(&rewritten).map_err(map_source_parse_error)?;
    let mut updated_module =
        select_one_module("", &updated_project.modules, &added_module.primary_selector)?;
    updated_module.source.clear();

    Ok((
        SourceMutationResult {
            action: "add-module".to_string(),
            family: String::new(),
            part_uri: String::new(),
            module: updated_module.clone(),
            previous_count: Some(project.modules.len()),
            module_count: Some(updated_project.modules.len()),
            previous_sha256: String::new(),
            sha256: updated_module.sha256.clone(),
            source_bytes: updated_module.source_bytes,
            line_count: updated_module.line_count,
            warnings,
            purged_caches: true,
            recompiles_on_open: true,
            office_load_verified: false,
            compatibility_status: "experimental".to_string(),
        },
        rewritten,
    ))
}

fn remove_module_source_in_project_data(
    file: &str,
    data: &[u8],
    selector: &str,
    expect_sha256: &str,
    options: SourceMutationOptions,
) -> CliResult<(SourceMutationResult, Vec<u8>)> {
    let project = parse_source_project(data).map_err(map_source_parse_error)?;
    if project.modules.len() <= 1 {
        return Err(CliError::invalid_args(
            "refusing to remove the last VBA module; use vba remove to remove the whole macro project",
        ));
    }
    let mut module = select_one_module(file, &project.modules, selector)?;
    let expected = normalize_sha256_guard(expect_sha256);
    if !expected.is_empty() && !expected.eq_ignore_ascii_case(&module.sha256) {
        return Err(CliError::invalid_args(format!(
            "VBA module source hash mismatch: expected {expected} but found {}",
            module.sha256
        )));
    }
    if module.stream_name.is_empty() {
        return Err(CliError::unexpected(format!(
            "VBA module {:?} has no stream name",
            module.name
        )));
    }
    for candidate in &project.modules {
        if candidate
            .primary_selector
            .eq_ignore_ascii_case(&module.primary_selector)
        {
            continue;
        }
        if candidate
            .stream_name
            .eq_ignore_ascii_case(&module.stream_name)
        {
            return Err(CliError::unexpected(format!(
                "refusing to remove VBA module {:?} because stream {:?} is shared by another module",
                module.name, module.stream_name
            )));
        }
    }

    let cfb_file = CfbFile::open(data).map_err(map_source_parse_error)?;
    require_experimental_source_rewrite_allowed(
        &project,
        &cfb_file,
        options.allow_experimental_source_rewrite,
    )?;
    if has_version_dependent_project_metadata(&cfb_file) {
        return Err(CliError::unexpected(
            "refusing to remove VBA module because this Office-shaped project has version-dependent _VBA_PROJECT metadata that must be regenerated for module-set changes; remove the whole macro project with vba remove, or create an Office-authored vbaProject.bin seed and attach it",
        ));
    }
    let dir_compressed = cfb_file
        .stream(DIR_STREAM_PATH)
        .map_err(CliError::unexpected)?;
    let dir_data = decompress_container(&dir_compressed).map_err(|err| {
        CliError::unexpected(format!("failed to decompress VBA dir stream: {err}"))
    })?;
    let removed_dir = remove_dir_module(&dir_data, &module)?;
    let streams = cfb_file.streams();
    let mut replacements = BTreeMap::new();
    replacements.insert(
        DIR_STREAM_PATH.to_string(),
        compress_container_literals(&removed_dir),
    );
    let mut warnings = Vec::new();
    if let Some(project_path) = optional_cfb_stream_path(&streams, "PROJECT") {
        let project_data = cfb_file
            .stream(&project_path)
            .map_err(CliError::unexpected)?;
        let (patched_project, removed_lines) =
            remove_project_stream_module_lines(&project_data, &module);
        if removed_lines == 0 {
            return Err(CliError::unexpected(format!(
                "PROJECT stream did not contain module entry lines for {}",
                module.name
            )));
        }
        replacements.insert(project_path, patched_project);
        warnings.push(format!(
            "removed {removed_lines} PROJECT stream line(s) for module {}",
            module.name
        ));
    } else {
        warnings
            .push("PROJECT stream was not present; skipped project metadata cleanup".to_string());
    }
    if let Some(project_wm_path) = optional_cfb_stream_path(&streams, "PROJECTwm") {
        let project_wm_data = cfb_file
            .stream(&project_wm_path)
            .map_err(CliError::unexpected)?;
        let (patched_project_wm, removed_entries) =
            remove_project_wm_module_entry(&project_wm_data, &module)?;
        if removed_entries == 0 {
            return Err(CliError::unexpected(format!(
                "PROJECTwm stream did not contain module entry for {}",
                module.name
            )));
        }
        replacements.insert(project_wm_path, patched_project_wm);
        warnings.push(format!(
            "removed {removed_entries} PROJECTwm entry(s) for module {}",
            module.name
        ));
    }
    let mut delete_streams = vba_compiled_cache_streams(&streams);
    delete_streams.push(format!("VBA/{}", module.stream_name));
    if delete_streams.len() > 1 {
        warnings.push(format!(
            "removed {} VBA compiled cache stream(s)",
            delete_streams.len() - 1
        ));
    }
    warnings.push("removed module metadata and stream; remaining module streams were preserved and Office compatibility remains unverified".to_string());
    let rewritten = rewrite_streams_with_deletes(data, &replacements, &delete_streams)
        .map_err(CliError::unexpected)?;
    let updated_project = parse_source_project(&rewritten).map_err(map_source_parse_error)?;
    if select_one_module("", &updated_project.modules, &module.primary_selector).is_ok() {
        return Err(CliError::unexpected(format!(
            "VBA module {} still exists after removal",
            module.primary_selector
        )));
    }
    module.source.clear();

    Ok((
        SourceMutationResult {
            action: "remove-module".to_string(),
            family: String::new(),
            part_uri: String::new(),
            module: module.clone(),
            previous_count: None,
            module_count: None,
            previous_sha256: module.sha256.clone(),
            sha256: String::new(),
            source_bytes: module.source_bytes,
            line_count: module.line_count,
            warnings,
            purged_caches: true,
            recompiles_on_open: true,
            office_load_verified: false,
            compatibility_status: "experimental".to_string(),
        },
        rewritten,
    ))
}

fn select_one_module(
    file: &str,
    modules: &[SourceModule],
    selector: &str,
) -> CliResult<SourceModule> {
    if selector.trim().is_empty() {
        return Err(CliError::invalid_args("module selector is required"));
    }
    let mut matches = select_modules(file, modules, selector)?;
    matches
        .pop()
        .ok_or_else(|| CliError::target_not_found(format!("VBA module not found: {selector}")))
}

fn normalize_sha256_guard(value: &str) -> String {
    value
        .trim()
        .strip_prefix("sha256:")
        .unwrap_or_else(|| value.trim())
        .to_ascii_lowercase()
}

fn encode_module_source(source: &[u8], code_page: i32) -> CliResult<(Vec<u8>, Vec<String>)> {
    let mut text = normalize_vba_line_endings(&String::from_utf8_lossy(source));
    let mut warnings = Vec::new();
    if !text.ends_with("\r\n") {
        text.push_str("\r\n");
        warnings.push("appended trailing CRLF to VBA source".to_string());
    }
    if code_page == 65001 {
        return Ok((text.into_bytes(), warnings));
    }
    let mut out = Vec::with_capacity(text.len());
    for ch in text.chars() {
        if u32::from(ch) > 0xFF {
            return Err(CliError::invalid_args(format!(
                "VBA source contains character {ch:?} that cannot be encoded with code page {code_page}"
            )));
        }
        out.push(ch as u8);
    }
    Ok((out, warnings))
}

fn normalize_vba_line_endings(text: &str) -> String {
    text.replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\n', "\r\n")
}

fn source_sha256(encoded_source: &[u8], code_page: i32) -> String {
    let decoded = decode_module_source(encoded_source, code_page);
    let mut hasher = Sha256::new();
    hasher.update(decoded.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn compress_container_literals(mut raw: &[u8]) -> Vec<u8> {
    let mut out = vec![0x01];
    while raw.len() >= 4096 {
        let header = 0x3000_u16 | 0x0FFF;
        out.extend(header.to_le_bytes());
        out.extend_from_slice(&raw[..4096]);
        raw = &raw[4096..];
    }
    if raw.is_empty() {
        return out;
    }
    while !raw.is_empty() {
        let literal_len = raw.len().min(3600);
        let literal_chunk = &raw[..literal_len];
        let mut chunk = Vec::new();
        let mut offset = 0;
        while offset < literal_chunk.len() {
            let n = (literal_chunk.len() - offset).min(8);
            chunk.push(0x00);
            chunk.extend_from_slice(&literal_chunk[offset..offset + n]);
            offset += n;
        }
        let header = ((chunk.len() - 1) as u16) | 0x3000 | 0x8000;
        out.extend(header.to_le_bytes());
        out.extend(chunk);
        raw = &raw[literal_len..];
    }
    out
}

fn require_experimental_source_rewrite_allowed(
    project: &SourceProject,
    cfb_file: &CfbFile<'_>,
    allowed: bool,
) -> CliResult<()> {
    if allowed {
        return Ok(());
    }
    let mut reasons = Vec::new();
    if let Some(module) = project
        .modules
        .iter()
        .find(|module| module.source_offset > 0)
    {
        reasons.push(format!(
            "module {} has non-zero MODULEOFFSET {}",
            module.name, module.source_offset
        ));
    }
    let caches = vba_compiled_cache_streams(&cfb_file.streams());
    if !caches.is_empty() {
        reasons.push(format!("{} compiled-cache stream(s) present", caches.len()));
    }
    if reasons.is_empty() {
        return Ok(());
    }
    Err(CliError::unexpected(format!(
        "experimental VBA source rewrite refused for Office-shaped project ({}); rerun with --allow-experimental-vba-source-rewrite after backing up and accepting that Office-load compatibility is not verified",
        reasons.join("; ")
    )))
}

fn has_version_dependent_project_metadata(cfb_file: &CfbFile<'_>) -> bool {
    cfb_file
        .stream("VBA/_VBA_PROJECT")
        .is_ok_and(|data| data.len() > 16)
}

fn rewrite_dir_module_offset(
    data: &[u8],
    target: &SourceModule,
    offset: u32,
) -> CliResult<(Vec<u8>, usize)> {
    let mut out = data.to_vec();
    let record = find_project_modules_record(&out).map_err(CliError::unexpected)?;
    let mut pos = record.modules_start;
    let mut patched = 0;
    for _ in 0..record.count {
        let mut current_module = DirModule::default();
        while out.len().saturating_sub(pos) >= 2 {
            let record_id = read_u16(&out, pos).map_err(CliError::unexpected)?;
            if record_id == 0x002B {
                if out.len().saturating_sub(pos) < 6 {
                    return Err(CliError::unexpected("module terminator is truncated"));
                }
                pos += 6;
                break;
            }
            if out.len().saturating_sub(pos) < 6 {
                return Err(CliError::unexpected(format!(
                    "module record 0x{record_id:04x} is truncated"
                )));
            }
            let record_size = read_u32(&out, pos + 2).map_err(CliError::unexpected)? as usize;
            let payload_start = pos + 6;
            let payload_end = payload_start + record_size;
            if payload_end > out.len() {
                return Err(CliError::unexpected(format!(
                    "module record 0x{record_id:04x} exceeds dir stream size"
                )));
            }
            if record_id == 0x0031 && dir_module_matches_source_module(&current_module, target) {
                if record_size < 4 {
                    return Err(CliError::unexpected("MODULEOFFSET record is too short"));
                }
                out[payload_start..payload_start + 4].copy_from_slice(&offset.to_le_bytes());
                patched += 1;
            }
            let payload = &out[payload_start..payload_end];
            match record_id {
                0x0019 => current_module.name = decode_mbcs(payload, 1252),
                0x0047 => {
                    let name = decode_utf16_le(payload);
                    if !name.is_empty() {
                        current_module.name = name;
                    }
                }
                0x001A => current_module.stream_name = decode_mbcs(payload, 1252),
                0x0032 => {
                    let name = decode_utf16_le(payload);
                    if !name.is_empty() {
                        current_module.stream_name = name;
                    }
                }
                _ => {}
            }
            pos = payload_end;
        }
    }
    Ok((out, patched))
}

fn remove_dir_module(data: &[u8], module: &SourceModule) -> CliResult<Vec<u8>> {
    let record = find_project_modules_record(data).map_err(CliError::unexpected)?;
    if record.count <= 1 {
        return Err(CliError::invalid_args(
            "refusing to remove the last VBA module",
        ));
    }
    let mut scan = record.modules_start;
    let mut remove_range = None;
    for _ in 0..record.count {
        let block_start = scan;
        let (dir_module, block_end) =
            read_dir_module_block(data, scan).map_err(CliError::unexpected)?;
        if dir_module_matches_source_module(&dir_module, module) {
            remove_range = Some((block_start, block_end));
            break;
        }
        scan = block_end;
    }
    let Some((remove_start, remove_end)) = remove_range else {
        return Err(CliError::unexpected(format!(
            "VBA module {} was not found in PROJECTMODULES records",
            module.primary_selector
        )));
    };
    let mut out = Vec::with_capacity(data.len() - (remove_end - remove_start));
    out.extend_from_slice(&data[..remove_start]);
    out.extend_from_slice(&data[remove_end..]);
    out[record.count_payload..record.count_payload + 2]
        .copy_from_slice(&((record.count - 1) as u16).to_le_bytes());
    Ok(out)
}

fn add_dir_module(data: &[u8], module: &SourceModule) -> CliResult<Vec<u8>> {
    let record = find_project_modules_record(data).map_err(CliError::unexpected)?;
    let module_block = build_dir_module_block(module);
    let mut out = Vec::with_capacity(data.len() + module_block.len());
    out.extend_from_slice(&data[..record.modules_end]);
    out.extend_from_slice(&module_block);
    out.extend_from_slice(&data[record.modules_end..]);
    out[record.count_payload..record.count_payload + 2]
        .copy_from_slice(&((record.count + 1) as u16).to_le_bytes());
    Ok(out)
}

fn build_dir_module_block(module: &SourceModule) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend(vba_dir_record(0x0019, module.name.as_bytes()));
    out.extend(vba_dir_record(0x0047, &utf16le_bytes(&module.name)));
    out.extend(vba_dir_record(0x001A, module.stream_name.as_bytes()));
    out.extend(vba_dir_record(0x0032, &utf16le_bytes(&module.stream_name)));
    out.extend(vba_dir_record(0x001C, &[]));
    out.extend(vba_dir_record(0x0048, &[]));
    out.extend(vba_dir_record(0x0031, &0_u32.to_le_bytes()));
    out.extend(vba_dir_record(0x001E, &0_u32.to_le_bytes()));
    out.extend(vba_dir_record(0x002C, &0xFFFF_u16.to_le_bytes()));
    if module.kind == "class" {
        out.extend(vba_dir_record(0x0022, &[]));
    } else {
        out.extend(vba_dir_record(0x0021, &[]));
    }
    out.extend(vba_dir_record(0x002B, &[]));
    out
}

fn vba_dir_record(id: u16, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(6 + payload.len());
    out.extend(id.to_le_bytes());
    out.extend((payload.len() as u32).to_le_bytes());
    out.extend(payload);
    out
}

fn utf16le_bytes(text: &str) -> Vec<u8> {
    text.encode_utf16()
        .flat_map(|unit| unit.to_le_bytes())
        .collect()
}

fn dir_module_matches_source_module(candidate: &DirModule, module: &SourceModule) -> bool {
    if !module.stream_name.is_empty()
        && candidate
            .stream_name
            .eq_ignore_ascii_case(&module.stream_name)
    {
        return true;
    }
    !module.name.is_empty() && candidate.name.eq_ignore_ascii_case(&module.name)
}

fn resolve_added_module_name(source: &[u8], requested: &str) -> CliResult<String> {
    let requested = requested.trim();
    let attr_name = module_attribute_name(source);
    if !requested.is_empty() {
        if !attr_name.is_empty() && !requested.eq_ignore_ascii_case(&attr_name) {
            return Err(CliError::invalid_args(format!(
                "requested module name {requested:?} does not match Attribute VB_Name {attr_name:?}"
            )));
        }
        return Ok(requested.to_string());
    }
    if !attr_name.is_empty() {
        return Ok(attr_name);
    }
    Err(CliError::invalid_args(
        "module name is required when source lacks Attribute VB_Name",
    ))
}

fn module_attribute_name(source: &[u8]) -> String {
    let text = String::from_utf8_lossy(source)
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    for line in text.split('\n') {
        let trimmed = line.trim();
        if !trimmed
            .to_ascii_lowercase()
            .starts_with("attribute vb_name")
        {
            continue;
        }
        let Some((_, value)) = trimmed.split_once('=') else {
            continue;
        };
        return value.trim().trim_matches('"').to_string();
    }
    String::new()
}

fn normalize_added_module_kind(kind: &str) -> CliResult<String> {
    let kind = kind.trim().to_ascii_lowercase();
    match kind.as_str() {
        "" | "standard" | "bas" | ".bas" => Ok("standard".to_string()),
        "class" | "cls" | ".cls" => Ok("class".to_string()),
        _ => Err(CliError::invalid_args(format!(
            "invalid VBA module kind {kind:?} (must be standard or class)"
        ))),
    }
}

fn normalize_replacement_module_kind(kind: &str) -> CliResult<String> {
    let kind = kind.trim().to_ascii_lowercase();
    match kind.as_str() {
        "" => Ok(String::new()),
        "standard" | "bas" | ".bas" => Ok("standard".to_string()),
        "class" | "cls" | ".cls" => Ok("class".to_string()),
        _ => Err(CliError::invalid_args(format!(
            "invalid replacement VBA module kind {kind:?} (must be standard or class)"
        ))),
    }
}

fn validate_replacement_module_source(
    target: &SourceModule,
    source: &[u8],
    source_kind: &str,
) -> CliResult<()> {
    let attr_name = module_attribute_name(source);
    if !attr_name.is_empty() && !attr_name.eq_ignore_ascii_case(&target.name) {
        return Err(CliError::invalid_args(format!(
            "replacement source Attribute VB_Name {attr_name:?} does not match target module {:?}",
            target.name
        )));
    }
    let kind = normalize_replacement_module_kind(source_kind)?;
    if !kind.is_empty() && !kind.eq_ignore_ascii_case(&target.kind) {
        return Err(CliError::invalid_args(format!(
            "replacement source kind {kind:?} is incompatible with target module {:?} kind {:?}",
            target.name, target.kind
        )));
    }
    Ok(())
}

fn validate_added_module_name(name: &str) -> CliResult<()> {
    let name = name.trim();
    if name.is_empty() {
        return Err(CliError::invalid_args("module name is required"));
    }
    if name
        .chars()
        .any(|ch| matches!(ch, '/' | '\\' | ':' | '"' | '[' | ']'))
    {
        return Err(CliError::invalid_args(format!(
            "module name {name:?} contains unsupported characters"
        )));
    }
    if name.encode_utf16().count() > 31 {
        return Err(CliError::invalid_args(format!(
            "module name {name:?} is longer than 31 UTF-16 code units"
        )));
    }
    Ok(())
}

fn prepare_added_module_source(
    source: &[u8],
    name: &str,
    code_page: i32,
) -> CliResult<(Vec<u8>, Vec<String>)> {
    let mut text = String::from_utf8_lossy(source).to_string();
    let mut warnings = Vec::new();
    if module_attribute_name(source).is_empty() {
        text = format!("Attribute VB_Name = \"{name}\"\r\n{text}");
        warnings.push("prepended Attribute VB_Name to VBA source".to_string());
    }
    let (encoded, encode_warnings) = encode_module_source(text.as_bytes(), code_page)?;
    warnings.extend(encode_warnings);
    Ok((encoded, warnings))
}

fn optional_cfb_stream_path(paths: &[String], want: &str) -> Option<String> {
    let path = find_cfb_stream_path(paths, want);
    (!path.is_empty()).then_some(path)
}

fn remove_project_stream_module_lines(data: &[u8], module: &SourceModule) -> (Vec<u8>, usize) {
    if data.is_empty() {
        return (Vec::new(), 0);
    }
    let text = String::from_utf8_lossy(data);
    let line_ending = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let trailing = text.ends_with('\n');
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut lines = normalized.split('\n').collect::<Vec<_>>();
    if lines.last() == Some(&"") {
        lines.pop();
    }
    let mut kept = Vec::new();
    let mut removed = 0;
    for line in lines {
        if is_project_stream_module_line(line, module) {
            removed += 1;
        } else {
            kept.push(line.to_string());
        }
    }
    if removed == 0 {
        return (data.to_vec(), 0);
    }
    let mut out = kept.join(line_ending);
    if trailing || !kept.is_empty() {
        out.push_str(line_ending);
    }
    (out.into_bytes(), removed)
}

fn add_project_stream_module_lines(
    data: &[u8],
    module: &SourceModule,
) -> CliResult<(Vec<u8>, usize)> {
    let text = String::from_utf8_lossy(data);
    let line_ending = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let trailing = text.ends_with('\n');
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut lines = normalized
        .split('\n')
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    if lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    for line in &lines {
        if is_project_stream_module_line(line, module) {
            return Err(CliError::unexpected(format!(
                "PROJECT stream already contains module entry for {}",
                module.name
            )));
        }
    }
    let module_line = if module.kind == "class" {
        format!("Class={}", module.name)
    } else {
        format!("Module={}", module.name)
    };
    let workspace_line = format!("{}=0, 0, 0, 0, C", module.name);
    let mut insert_at = lines.len();
    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        let lower = trimmed.to_ascii_lowercase();
        if lower == "[workspace]" || trimmed.starts_with('[') || lower.starts_with("name=") {
            insert_at = idx;
            break;
        }
        if project_stream_module_declaration_key(line) {
            insert_at = idx + 1;
        }
    }
    let mut out_lines = Vec::with_capacity(lines.len() + 3);
    out_lines.extend_from_slice(&lines[..insert_at]);
    out_lines.push(module_line);
    out_lines.extend_from_slice(&lines[insert_at..]);

    let mut workspace_at = None;
    let mut workspace_end = out_lines.len();
    for (idx, line) in out_lines.iter().enumerate() {
        if !line.trim().eq_ignore_ascii_case("[Workspace]") {
            continue;
        }
        workspace_at = Some(idx);
        workspace_end = idx + 1;
        for (scan, candidate) in out_lines.iter().enumerate().skip(idx + 1) {
            if candidate.trim().starts_with('[') {
                break;
            }
            workspace_end = scan + 1;
        }
        break;
    }
    if workspace_at.is_some() {
        out_lines.insert(workspace_end, workspace_line);
    } else {
        out_lines.push("[Workspace]".to_string());
        out_lines.push(workspace_line);
    }
    let added = out_lines.len() - lines.len();
    let mut out = out_lines.join(line_ending);
    if trailing || !out_lines.is_empty() {
        out.push_str(line_ending);
    }
    Ok((out.into_bytes(), added))
}

fn project_stream_module_declaration_key(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('[') {
        return false;
    }
    let Some((key, _)) = trimmed.split_once('=') else {
        return false;
    };
    matches!(
        key.trim().to_ascii_lowercase().as_str(),
        "document" | "module" | "class" | "baseclass"
    )
}

fn is_project_stream_module_line(line: &str, module: &SourceModule) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('[') {
        return false;
    }
    let Some((key, mut value)) = trimmed.split_once('=') else {
        return false;
    };
    let key = key.trim().to_ascii_lowercase();
    value = value.trim();
    let names = [module.name.as_str(), module.stream_name.as_str()];
    match key.as_str() {
        "module" | "class" | "baseclass" => matches_any_module_name(value, &names),
        "document" => {
            if let Some((before, _)) = value.split_once('/') {
                value = before;
            }
            matches_any_module_name(value, &names)
        }
        _ => matches_any_module_name(key.trim(), &names),
    }
}

#[derive(Clone)]
struct ProjectWmModuleEntry {
    name: String,
    display_name: String,
}

fn add_project_wm_module_entry(data: &[u8], module: &SourceModule) -> CliResult<(Vec<u8>, usize)> {
    let mut entries = parse_project_wm_module_entries(data)?;
    for entry in &entries {
        if project_wm_entry_matches_module(entry, module) {
            return Err(CliError::unexpected(format!(
                "PROJECTwm stream already contains module entry for {}",
                module.name
            )));
        }
    }
    entries.push(ProjectWmModuleEntry {
        name: module.name.clone(),
        display_name: module.name.clone(),
    });
    Ok((build_project_wm_module_entries(&entries), 1))
}

fn remove_project_wm_module_entry(
    data: &[u8],
    module: &SourceModule,
) -> CliResult<(Vec<u8>, usize)> {
    let entries = parse_project_wm_module_entries(data)?;
    let mut kept = Vec::new();
    let mut removed = 0;
    for entry in entries {
        if project_wm_entry_matches_module(&entry, module) {
            removed += 1;
        } else {
            kept.push(entry);
        }
    }
    if removed == 0 {
        return Ok((data.to_vec(), 0));
    }
    Ok((build_project_wm_module_entries(&kept), removed))
}

fn parse_project_wm_module_entries(data: &[u8]) -> CliResult<Vec<ProjectWmModuleEntry>> {
    let mut entries = Vec::new();
    let mut pos = 0;
    while pos < data.len() {
        if data[pos] == 0 {
            if pos + 1 < data.len() && data[pos + 1] == 0 {
                return Ok(entries);
            }
            return Err(CliError::unexpected(format!(
                "PROJECTwm stream has an empty module name at byte {pos}"
            )));
        }
        let name_start = pos;
        while pos < data.len() && data[pos] != 0 {
            pos += 1;
        }
        if pos >= data.len() {
            return Err(CliError::unexpected(
                "PROJECTwm stream module name is unterminated",
            ));
        }
        let name = String::from_utf8_lossy(&data[name_start..pos]).to_string();
        pos += 1;
        let display_start = pos;
        loop {
            if pos + 1 >= data.len() {
                return Err(CliError::unexpected(format!(
                    "PROJECTwm stream display name for {name} is unterminated"
                )));
            }
            if data[pos] == 0 && data[pos + 1] == 0 {
                let display_name = decode_utf16_le(&data[display_start..pos]);
                pos += 2;
                entries.push(ProjectWmModuleEntry { name, display_name });
                break;
            }
            pos += 2;
        }
    }
    Ok(entries)
}

fn build_project_wm_module_entries(entries: &[ProjectWmModuleEntry]) -> Vec<u8> {
    let mut out = Vec::new();
    for entry in entries {
        let display_name = if entry.display_name.is_empty() {
            &entry.name
        } else {
            &entry.display_name
        };
        out.extend_from_slice(entry.name.as_bytes());
        out.push(0);
        out.extend(utf16le_bytes(display_name));
        out.extend([0, 0]);
    }
    out.extend([0, 0]);
    out
}

fn project_wm_entry_matches_module(entry: &ProjectWmModuleEntry, module: &SourceModule) -> bool {
    let names = [module.name.as_str(), module.stream_name.as_str()];
    matches_any_module_name(&entry.name, &names)
        || matches_any_module_name(&entry.display_name, &names)
}

fn matches_any_module_name(value: &str, names: &[&str]) -> bool {
    let value = value.trim_matches('"');
    names
        .iter()
        .any(|name| !name.trim().is_empty() && value.eq_ignore_ascii_case(name.trim()))
}

fn vba_compiled_cache_streams(paths: &[String]) -> Vec<String> {
    paths
        .iter()
        .filter(|path| {
            path.replace('\\', "/")
                .to_ascii_lowercase()
                .starts_with("vba/__srp_")
        })
        .cloned()
        .collect()
}

fn source_mutation_result_json(result: &SourceMutationResult) -> Value {
    let mut object = Map::new();
    object.insert("action".to_string(), json!(result.action));
    if !result.family.is_empty() {
        object.insert("family".to_string(), json!(result.family));
    }
    if !result.part_uri.is_empty() {
        object.insert("partUri".to_string(), json!(result.part_uri));
    }
    object.insert(
        "module".to_string(),
        source_module_json(&result.module, false),
    );
    if let Some(previous_count) = result.previous_count {
        object.insert("previousCount".to_string(), json!(previous_count));
    }
    if let Some(module_count) = result.module_count {
        object.insert("moduleCount".to_string(), json!(module_count));
    }
    if !result.previous_sha256.is_empty() {
        object.insert("previousSha256".to_string(), json!(result.previous_sha256));
    }
    if !result.sha256.is_empty() {
        object.insert("sha256".to_string(), json!(result.sha256));
    }
    if let Some(source_bytes) = result.source_bytes {
        object.insert("sourceBytes".to_string(), json!(source_bytes));
    }
    if let Some(line_count) = result.line_count {
        object.insert("lineCount".to_string(), json!(line_count));
    }
    if !result.warnings.is_empty() {
        object.insert("warnings".to_string(), json!(result.warnings));
    }
    object.insert("purgedCaches".to_string(), json!(result.purged_caches));
    object.insert(
        "recompilesOnOpen".to_string(),
        json!(result.recompiles_on_open),
    );
    object.insert(
        "officeLoadVerified".to_string(),
        json!(result.office_load_verified),
    );
    if !result.compatibility_status.is_empty() {
        object.insert(
            "compatibilityStatus".to_string(),
            json!(result.compatibility_status),
        );
    }
    Value::Object(object)
}

fn vba_source_mutation_output_json(
    file: &str,
    output: Option<&str>,
    dry_run: bool,
    result: &SourceMutationResult,
    info: &VbaInfo,
    project: &SourceProject,
    target: &str,
) -> Value {
    let mut object = Map::new();
    object.insert("file".to_string(), json!(file));
    if let Some(output) = output.filter(|value| !value.trim().is_empty()) {
        object.insert("output".to_string(), json!(output));
    }
    object.insert("dryRun".to_string(), json!(dry_run));
    object.insert("result".to_string(), source_mutation_result_json(result));
    object.insert("vba".to_string(), vba_info_json(info));
    object.insert("project".to_string(), source_project_json(project, false));
    let module_selector = result.module.primary_selector.as_str();
    if dry_run {
        object.insert(
            "inspectCommandTemplate".to_string(),
            json!(vba_inspect_command(target)),
        );
        object.insert(
            "validateCommandTemplate".to_string(),
            json!(vba_validate_command(target)),
        );
        if let Some(command) = vba_office_check_command(target, info) {
            object.insert("officeCheckCommandTemplate".to_string(), json!(command));
        }
        object.insert(
            "packageReadbackCommandTemplate".to_string(),
            json!(vba_package_readback_command(target, info.family.family)),
        );
        object.insert(
            "listCommandTemplate".to_string(),
            json!(vba_list_command(target)),
        );
        if result.action != "remove-module" {
            object.insert(
                "extractCommandTemplate".to_string(),
                json!(vba_extract_module_command(target, module_selector)),
            );
        }
    } else {
        object.insert(
            "inspectCommand".to_string(),
            json!(vba_inspect_command(target)),
        );
        object.insert(
            "validateCommand".to_string(),
            json!(vba_validate_command(target)),
        );
        if let Some(command) = vba_office_check_command(target, info) {
            object.insert("officeCheckCommand".to_string(), json!(command));
        }
        object.insert(
            "packageReadbackCommand".to_string(),
            json!(vba_package_readback_command(target, info.family.family)),
        );
        object.insert("listCommand".to_string(), json!(vba_list_command(target)));
        if result.action != "remove-module" {
            object.insert(
                "extractCommand".to_string(),
                json!(vba_extract_module_command(target, module_selector)),
            );
        }
    }
    Value::Object(object)
}

fn vba_extract_module_command(file: &str, module_selector: &str) -> String {
    let mut command = format!(
        "ooxml --json vba extract {} --out-dir macros",
        command_arg(file)
    );
    if !module_selector.trim().is_empty() {
        command.push_str(&format!(" --module {}", command_arg(module_selector)));
    }
    command
}

fn inspect_source_project_for_file(file: &str) -> CliResult<(VbaInfo, SourceProject)> {
    let info = inspect_vba_package(file)?;
    let project = info
        .project
        .as_ref()
        .filter(|project| project.exists)
        .ok_or_else(|| {
            missing_vba_project_error(file, &info, "package has no vbaProject.bin part")
        })?;
    let part_name = package_part_name(&project.part_uri);
    let data = zip_bytes(file, &part_name)?;
    let mut source_project = parse_source_project_for_family(&data, info.family.family)?;
    source_project.part_uri = project.part_uri.clone();
    Ok((info, source_project))
}

fn missing_vba_project_error(file: &str, info: &VbaInfo, message: &str) -> CliError {
    CliError::target_not_found(format!(
        "{message}; inspect macro state with `{}`; attach a VBA project with `{}`",
        vba_inspect_command(file),
        vba_next_mutation_template(file, info)
    ))
}

fn parse_source_project_for_family(data: &[u8], family: &str) -> CliResult<SourceProject> {
    let mut project = parse_source_project(data).map_err(map_source_parse_error)?;
    project.family = family.to_ascii_lowercase();
    populate_office_compatibility(&mut project);
    Ok(project)
}

fn normalize_inspect_bin_family(value: &str) -> CliResult<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "" => Err(CliError::invalid_args(
            "--family is required for inspect-bin (pptx or xlsx)",
        )),
        "pptx" | "pptm" | "powerpoint" | "presentation" => Ok("pptx".to_string()),
        "xlsx" | "xlsm" | "excel" | "workbook" => Ok("xlsx".to_string()),
        _ => Err(CliError::invalid_args("--family must be pptx or xlsx")),
    }
}

fn map_source_parse_error(message: String) -> CliError {
    if message.contains("Compound File Binary")
        || message.contains("VBA dir stream")
        || message.contains("compressed")
        || message.contains("PROJECTMODULES")
        || message.contains("is empty")
    {
        return CliError::invalid_args(message);
    }
    CliError::unexpected(message)
}

fn parse_source_project(data: &[u8]) -> Result<SourceProject, String> {
    let cfb_file = CfbFile::open(data)?;
    let dir_compressed = cfb_file
        .stream(DIR_STREAM_PATH)
        .map_err(|err| format!("failed to read VBA dir stream: {err}"))?;
    let dir_data = decompress_container(&dir_compressed)
        .map_err(|err| format!("failed to decompress VBA dir stream: {err}"))?;
    let (code_page, modules, warnings) = parse_dir_stream(&dir_data)?;

    let mut project = SourceProject {
        family: String::new(),
        part_uri: String::new(),
        code_page,
        module_count: modules.len(),
        modules: Vec::new(),
        project_metadata: parse_project_metadata(&cfb_file, code_page),
        office_compatibility: OfficeCompatibilityReport {
            office_load_verified: false,
            status: "unverified".to_string(),
            risks: Vec::new(),
            notes: Vec::new(),
        },
        host_compatibility_warnings: Vec::new(),
        warnings,
    };

    for (idx, module) in modules.into_iter().enumerate() {
        let mut item = SourceModule {
            number: idx + 1,
            name: module.name,
            stream_name: module.stream_name,
            kind: module.kind,
            extension: String::new(),
            code_page,
            source_offset: module.source_offset,
            source_bytes: None,
            line_count: None,
            sha256: String::new(),
            sha256_basis: String::new(),
            line_ending: "none".to_string(),
            trailing_newline: false,
            primary_selector: String::new(),
            selectors: Vec::new(),
            source: String::new(),
            warnings: Vec::new(),
        };
        item.extension = extension_for_module_kind(&item.kind).to_string();
        if item.name.is_empty() {
            item.name.clone_from(&item.stream_name);
        }
        if item.kind.is_empty() {
            item.kind = "unknown".to_string();
            item.extension = ".bas".to_string();
            item.warnings
                .push("module type was not present in dir stream".to_string());
        }
        let stream_path = format!("VBA/{}", item.stream_name);
        match cfb_file.stream(&stream_path) {
            Err(err) => item.warnings.push(err),
            Ok(stream_data) if item.source_offset as usize > stream_data.len() => {
                item.warnings.push(format!(
                    "source offset {} exceeds module stream size {}",
                    item.source_offset,
                    stream_data.len()
                ));
            }
            Ok(stream_data) => {
                let source_compressed = &stream_data[item.source_offset as usize..];
                match decompress_container(source_compressed) {
                    Err(err) => item
                        .warnings
                        .push(format!("failed to decompress module source: {err}")),
                    Ok(source_bytes) => {
                        let source = decode_module_source(&source_bytes, code_page);
                        item.source_bytes = Some(source.len());
                        item.line_count = Some(count_source_lines(&source));
                        item.line_ending = source_line_ending_style(&source).to_string();
                        item.trailing_newline = source_has_trailing_line_ending(&source);
                        let mut hasher = Sha256::new();
                        hasher.update(source.as_bytes());
                        item.sha256 = format!("{:x}", hasher.finalize());
                        item.sha256_basis = "decoded-source-utf8".to_string();
                        item.source = source;
                    }
                }
            }
        }
        project.modules.push(with_source_module_selectors(item));
    }
    project.modules.sort_by_key(|module| module.number);
    Ok(project)
}

fn parse_project_metadata(cfb_file: &CfbFile<'_>, code_page: i32) -> Option<ProjectMetadata> {
    let streams = cfb_file.streams();
    let project_wm_path = find_cfb_stream_path(&streams, "PROJECTwm");
    let project_path = find_cfb_stream_path(&streams, "PROJECT");
    let has_project_wm = !project_wm_path.is_empty();
    if project_path.is_empty() {
        if has_project_wm {
            return Some(ProjectMetadata {
                stream_name: String::new(),
                present: false,
                line_count: 0,
                id: String::new(),
                name: String::new(),
                modules: Vec::new(),
                references: Vec::new(),
                workspace_entries: Vec::new(),
                has_project_wm,
                project_wm_stream: project_wm_path,
                warnings: vec![
                    "PROJECTwm stream exists but PROJECT stream was not found".to_string(),
                ],
            });
        }
        return None;
    }
    let mut metadata = ProjectMetadata {
        stream_name: project_path.clone(),
        present: true,
        line_count: 0,
        id: String::new(),
        name: String::new(),
        modules: Vec::new(),
        references: Vec::new(),
        workspace_entries: Vec::new(),
        has_project_wm,
        project_wm_stream: project_wm_path,
        warnings: Vec::new(),
    };
    let data = match cfb_file.stream(&project_path) {
        Ok(data) => data,
        Err(err) => {
            metadata.warnings.push(err);
            return Some(metadata);
        }
    };
    let text = decode_mbcs(&data, code_page)
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    let mut lines = text.split('\n').collect::<Vec<_>>();
    if lines.last() == Some(&"") {
        lines.pop();
    }
    metadata.line_count = lines.len();
    let mut in_workspace = false;
    for (idx, line) in lines.iter().enumerate() {
        let line_no = idx + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_workspace = trimmed.eq_ignore_ascii_case("[Workspace]");
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        let lower_key = key.to_ascii_lowercase();
        if in_workspace {
            metadata.workspace_entries.push(ProjectWorkspaceEntry {
                name: key.to_string(),
                value: value.to_string(),
                line: line_no,
            });
            continue;
        }
        match lower_key.as_str() {
            "id" => metadata.id = value.trim_matches('"').to_string(),
            "name" => metadata.name = value.trim_matches('"').to_string(),
            "module" | "class" | "baseclass" | "document" => {
                let mut name = value.to_string();
                if lower_key == "document"
                    && let Some((before, _)) = value.split_once('/')
                {
                    name = before.to_string();
                }
                metadata.modules.push(ProjectModuleDeclaration {
                    kind: lower_key,
                    name: name.trim_matches('"').to_string(),
                    value: value.to_string(),
                    line: line_no,
                });
            }
            "reference" | "object" | "package" | "control" => {
                metadata.references.push(ProjectReference {
                    kind: lower_key,
                    value: value.to_string(),
                    line: line_no,
                });
            }
            _ => {}
        }
    }
    Some(metadata)
}

fn decompress_container(data: &[u8]) -> Result<Vec<u8>, String> {
    if data.is_empty() {
        return Err("compressed container is empty".to_string());
    }
    if data[0] != 0x01 {
        return Err(format!(
            "compressed container signature 0x{:02x}, want 0x01",
            data[0]
        ));
    }
    let mut out = Vec::new();
    let mut pos = 1;
    while pos < data.len() {
        if pos + 2 > data.len() {
            return Err("truncated compressed chunk header".to_string());
        }
        let header = u16::from_le_bytes([data[pos], data[pos + 1]]);
        if header == 0 {
            break;
        }
        if header & 0x7000 != 0x3000 {
            return Err(format!(
                "invalid compressed chunk signature in header 0x{header:04x}"
            ));
        }
        let chunk_size = usize::from(header & 0x0FFF) + 3;
        let chunk_end = pos + chunk_size;
        if chunk_end > data.len() {
            return Err("compressed chunk exceeds stream size".to_string());
        }
        let compressed = header & 0x8000 != 0;
        let chunk_data = &data[pos + 2..chunk_end];
        let chunk_start = out.len();
        if !compressed {
            if chunk_data.len() != 4096 {
                return Err(format!(
                    "raw compressed chunk has {} bytes, want 4096",
                    chunk_data.len()
                ));
            }
            out.extend_from_slice(chunk_data);
        } else {
            decompress_chunk(chunk_data, chunk_start, &mut out)?;
        }
        pos = chunk_end;
    }
    Ok(out)
}

fn decompress_chunk(data: &[u8], chunk_start: usize, out: &mut Vec<u8>) -> Result<(), String> {
    let mut pos = 0;
    while pos < data.len() {
        let flags = data[pos];
        pos += 1;
        for bit in 0..8 {
            if pos >= data.len() {
                break;
            }
            if flags & (1 << bit) == 0 {
                out.push(data[pos]);
                pos += 1;
                continue;
            }
            if pos + 2 > data.len() {
                return Err("truncated copy token".to_string());
            }
            let token = u16::from_le_bytes([data[pos], data[pos + 1]]);
            pos += 2;
            let (offset, length) = unpack_copy_token(token, out.len() - chunk_start);
            if offset > out.len() || out.len() - offset < chunk_start {
                return Err(format!(
                    "copy token offset {offset} precedes decompressed chunk"
                ));
            }
            let copy_start = out.len() - offset;
            for i in 0..length {
                out.push(out[copy_start + i]);
            }
        }
    }
    Ok(())
}

fn unpack_copy_token(token: u16, difference: usize) -> (usize, usize) {
    let mut bit_count = 4;
    let mut limit = 16;
    while difference > limit && bit_count < 12 {
        bit_count += 1;
        limit <<= 1;
    }
    let length_bits = 16 - bit_count;
    let length_mask = (1_u16 << length_bits) - 1;
    let length = usize::from(token & length_mask) + 3;
    let offset = usize::from(token >> length_bits) + 1;
    (offset, length)
}

fn parse_dir_stream(data: &[u8]) -> Result<(i32, Vec<DirModule>, Vec<String>), String> {
    let modules_record = find_project_modules_record(data)?;
    let (code_page, found_code_page) = find_project_code_page(data, modules_record.record_start);
    let mut reader = DirReader {
        data,
        pos: modules_record.modules_start,
        code_page,
        modules: Vec::new(),
        warnings: Vec::new(),
    };
    reader.parse_modules(modules_record.count)?;
    if !found_code_page {
        reader.warnings.push(
            "PROJECTCODEPAGE record was not found before PROJECTMODULES; defaulted to Windows-1252"
                .to_string(),
        );
    }
    Ok((reader.code_page, reader.modules, reader.warnings))
}

impl<'a> DirReader<'a> {
    fn parse_modules(&mut self, count: usize) -> Result<(), String> {
        for _ in 0..count {
            let module = self.parse_module()?;
            self.modules.push(module);
        }
        Ok(())
    }

    fn parse_module(&mut self) -> Result<DirModule, String> {
        let mut module = DirModule::default();
        while self.remaining() >= 2 {
            let id = read_u16(self.data, self.pos)?;
            if id == 0x002B {
                if self.remaining() < 6 {
                    return Err("module terminator is truncated".to_string());
                }
                self.pos += 6;
                if module.stream_name.is_empty() {
                    module.stream_name.clone_from(&module.name);
                    self.warnings.push(format!(
                        "module {:?} did not include MODULESTREAMNAME",
                        module.name
                    ));
                }
                return Ok(module);
            }
            if self.remaining() < 6 {
                return Err(format!("module record 0x{id:04x} is truncated"));
            }
            let size = read_u32(self.data, self.pos + 2)? as usize;
            let payload_start = self.pos + 6;
            let payload_end = payload_start + size;
            if payload_end > self.data.len() {
                return Err(format!("module record 0x{id:04x} exceeds dir stream size"));
            }
            let payload = &self.data[payload_start..payload_end];
            match id {
                0x0019 => module.name = decode_mbcs(payload, self.code_page),
                0x0047 => {
                    let name = decode_utf16_le(payload);
                    if !name.is_empty() {
                        module.name = name;
                    }
                }
                0x001A => module.stream_name = decode_mbcs(payload, self.code_page),
                0x0032 => {
                    let name = decode_utf16_le(payload);
                    if !name.is_empty() {
                        module.stream_name = name;
                    }
                }
                0x0031 => {
                    if payload.len() < 4 {
                        return Err("MODULEOFFSET record is too short".to_string());
                    }
                    module.source_offset = read_u32(payload, 0)?;
                }
                0x0021 => module.kind = "standard".to_string(),
                0x0022 => module.kind = "class".to_string(),
                _ => {}
            }
            self.pos = payload_end;
        }
        Err("module record terminated unexpectedly".to_string())
    }

    fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }
}

fn find_project_modules_record(data: &[u8]) -> Result<ProjectModulesRecord, String> {
    for pos in 0..data.len().saturating_sub(7) {
        if read_u16(data, pos)? != 0x000F {
            continue;
        }
        if read_u32(data, pos + 2)? != 2 {
            continue;
        }
        let count = usize::from(read_u16(data, pos + 6)?);
        if count == 0 {
            continue;
        }
        let Ok(modules_start) = skip_project_cookie(data, pos + 8) else {
            continue;
        };
        let mut modules_end = modules_start;
        let mut ok = true;
        for _ in 0..count {
            match read_dir_module_block(data, modules_end) {
                Ok((_, block_end)) => modules_end = block_end,
                Err(_) => {
                    ok = false;
                    break;
                }
            }
        }
        if ok {
            return Ok(ProjectModulesRecord {
                record_start: pos,
                count_payload: pos + 6,
                count,
                modules_start,
                modules_end,
            });
        }
    }
    Err("PROJECTMODULES record not found in VBA dir stream".to_string())
}

fn read_dir_module_block(data: &[u8], mut pos: usize) -> Result<(DirModule, usize), String> {
    let mut module = DirModule::default();
    while data.len().saturating_sub(pos) >= 2 {
        let id = read_u16(data, pos)?;
        if id == 0x002B {
            if data.len().saturating_sub(pos) < 6 {
                return Err("module terminator is truncated".to_string());
            }
            if module.stream_name.is_empty() {
                module.stream_name.clone_from(&module.name);
            }
            return Ok((module, pos + 6));
        }
        if data.len().saturating_sub(pos) < 6 {
            return Err(format!("module record 0x{id:04x} is truncated"));
        }
        let size = read_u32(data, pos + 2)? as usize;
        let payload_start = pos + 6;
        let payload_end = payload_start + size;
        if payload_end > data.len() {
            return Err(format!("module record 0x{id:04x} exceeds dir stream size"));
        }
        let payload = &data[payload_start..payload_end];
        match id {
            0x0019 => module.name = decode_mbcs(payload, 1252),
            0x0047 => {
                let name = decode_utf16_le(payload);
                if !name.is_empty() {
                    module.name = name;
                }
            }
            0x001A => module.stream_name = decode_mbcs(payload, 1252),
            0x0032 => {
                let name = decode_utf16_le(payload);
                if !name.is_empty() {
                    module.stream_name = name;
                }
            }
            0x0031 => {
                if payload.len() < 4 {
                    return Err("MODULEOFFSET record is too short".to_string());
                }
                module.source_offset = read_u32(payload, 0)?;
            }
            0x0021 => module.kind = "standard".to_string(),
            0x0022 => module.kind = "class".to_string(),
            _ => {}
        }
        pos = payload_end;
    }
    Err("module record terminated unexpectedly".to_string())
}

fn find_project_code_page(data: &[u8], end: usize) -> (i32, bool) {
    let end = end.min(data.len());
    for pos in 0..end.saturating_sub(7) {
        if read_u16(data, pos).unwrap_or_default() != 0x0003 {
            continue;
        }
        if read_u32(data, pos + 2).unwrap_or_default() != 2 {
            continue;
        }
        let code_page = i32::from(read_u16(data, pos + 6).unwrap_or_default());
        if code_page > 0 {
            return (code_page, true);
        }
    }
    (1252, false)
}

fn skip_project_cookie(data: &[u8], pos: usize) -> Result<usize, String> {
    if data.len().saturating_sub(pos) < 6 || read_u16(data, pos)? != 0x0013 {
        return Ok(pos);
    }
    let size = read_u32(data, pos + 2)? as usize;
    let record_end = pos + 6 + size;
    if record_end > data.len() {
        return Err("PROJECTCOOKIE record exceeds dir stream size".to_string());
    }
    Ok(record_end)
}

fn decode_module_source(data: &[u8], code_page: i32) -> String {
    let end = data
        .iter()
        .rposition(|value| *value != 0)
        .map(|idx| idx + 1)
        .unwrap_or(0);
    decode_mbcs(&data[..end], code_page)
}

fn decode_mbcs(data: &[u8], code_page: i32) -> String {
    if data.is_empty() {
        return String::new();
    }
    if code_page == 65001 {
        return String::from_utf8_lossy(data).into_owned();
    }
    data.iter().map(|value| char::from(*value)).collect()
}

fn decode_utf16_le(data: &[u8]) -> String {
    let mut units = Vec::with_capacity(data.len() / 2);
    for chunk in data.chunks_exact(2) {
        let value = u16::from_le_bytes([chunk[0], chunk[1]]);
        if value == 0 {
            break;
        }
        units.push(value);
    }
    String::from_utf16_lossy(&units)
}

fn count_source_lines(source: &str) -> usize {
    if source.is_empty() {
        return 0;
    }
    let lines = source.matches('\n').count();
    if source.ends_with('\n') {
        lines
    } else {
        lines + 1
    }
}

fn source_line_ending_style(source: &str) -> &'static str {
    let mut has_crlf = false;
    let mut has_lf = false;
    let mut has_cr = false;
    let bytes = source.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'\r' if index + 1 < bytes.len() && bytes[index + 1] == b'\n' => {
                has_crlf = true;
                index += 2;
                continue;
            }
            b'\r' => has_cr = true,
            b'\n' => has_lf = true,
            _ => {}
        }
        index += 1;
    }
    let kinds = [has_crlf, has_lf, has_cr]
        .into_iter()
        .filter(|present| *present)
        .count();
    match (kinds, has_crlf, has_lf) {
        (0, _, _) => "none",
        (2.., _, _) => "mixed",
        (_, true, _) => "crlf",
        (_, _, true) => "lf",
        _ => "cr",
    }
}

fn source_has_trailing_line_ending(source: &str) -> bool {
    source.ends_with('\n') || source.ends_with('\r')
}

fn extension_for_module_kind(kind: &str) -> &'static str {
    match kind {
        "class" => ".cls",
        _ => ".bas",
    }
}

fn with_source_module_selectors(mut module: SourceModule) -> SourceModule {
    let mut builder = SelectorBuilder::default();
    if !module.name.trim().is_empty() {
        module.primary_selector = format!("module:{}", module.name);
    } else if module.number > 0 {
        module.primary_selector = format!("module:{}", module.number);
    }
    builder.add(&module.primary_selector);
    if module.number > 0 {
        builder.add(&format!("module:{}", module.number));
        builder.add(&format!("#{}", module.number));
    }
    if !module.name.trim().is_empty() {
        builder.add(&format!("module:{}", module.name));
        builder.add(&format!("name:{}", module.name));
        builder.add(&format!("~{}", module.name));
        builder.add(&module.name);
    }
    if !module.stream_name.trim().is_empty() {
        builder.add(&format!("stream:{}", module.stream_name));
    }
    module.selectors = builder.values;
    module
}

#[derive(Default)]
struct SelectorBuilder {
    values: Vec<String>,
    seen: BTreeMap<String, bool>,
}

impl SelectorBuilder {
    fn add(&mut self, value: &str) {
        let value = value.trim();
        if value.is_empty() {
            return;
        }
        let key = value.to_ascii_lowercase();
        if self.seen.contains_key(&key) {
            return;
        }
        self.seen.insert(key, true);
        self.values.push(value.to_string());
    }
}

fn module_output_name(module: &SourceModule) -> String {
    let mut name = if module.name.trim().is_empty() {
        module.stream_name.clone()
    } else {
        module.name.clone()
    };
    if name.trim().is_empty() {
        name = format!("module-{}", module.number);
    }
    name = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | ' ' | '-') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches([' ', '.'])
        .to_string();
    if name.is_empty() {
        name = format!("module-{}", module.number);
    }
    let extension = if module.extension.is_empty() {
        extension_for_module_kind(&module.kind)
    } else {
        &module.extension
    };
    let mut path = PathBuf::from(name);
    path.set_extension(extension.trim_start_matches('.'));
    path.file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("module.bas")
        .to_string()
}

fn select_modules(
    file: &str,
    modules: &[SourceModule],
    selector: &str,
) -> CliResult<Vec<SourceModule>> {
    let selector = selector.trim();
    if selector.is_empty() {
        return Ok(modules.to_vec());
    }
    let matches = modules
        .iter()
        .filter(|module| {
            module
                .selectors
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(selector))
        })
        .cloned()
        .collect::<Vec<_>>();
    match matches.len() {
        0 => Err(vba_module_not_found_error(file, modules, selector)),
        1 => Ok(matches),
        _ => Err(CliError::invalid_args(format!(
            "VBA module selector {selector:?} matched multiple modules ({}); use a more specific selector; discover with `{}`",
            ambiguous_module_selectors(&matches).join(", "),
            vba_list_command(file)
        ))),
    }
}

fn vba_module_not_found_error(file: &str, modules: &[SourceModule], selector: &str) -> CliError {
    let candidates = selector_candidates(
        &modules
            .iter()
            .map(|module| {
                (
                    module.primary_selector.as_str(),
                    module.selectors.as_slice(),
                )
            })
            .collect::<Vec<_>>(),
        selector,
        3,
    );
    let mut message = format!("VBA module not found: {selector}");
    if !candidates.is_empty() {
        message.push_str(&format!("; did you mean: {}", candidates.join(", ")));
    }
    message.push_str(&format!("; discover with `{}`", vba_list_command(file)));
    CliError::target_not_found(message)
}

fn ambiguous_module_selectors(modules: &[SourceModule]) -> Vec<String> {
    let mut primary_counts = BTreeMap::<String, usize>::new();
    for module in modules {
        let primary = module.primary_selector.trim();
        if !primary.is_empty() {
            *primary_counts
                .entry(primary.to_ascii_lowercase())
                .or_insert(0) += 1;
        }
    }
    modules
        .iter()
        .filter_map(|module| {
            let primary = module.primary_selector.trim();
            if !primary.is_empty()
                && primary_counts
                    .get(&primary.to_ascii_lowercase())
                    .copied()
                    .unwrap_or_default()
                    == 1
            {
                return Some(primary.to_string());
            }
            if module.number > 0 {
                return Some(format!("module:{}", module.number));
            }
            if !primary.is_empty() {
                return Some(primary.to_string());
            }
            None
        })
        .collect()
}

fn populate_office_compatibility(project: &mut SourceProject) {
    let host_warnings = host_compatibility_warnings(project);
    for warning in &host_warnings {
        if !project
            .warnings
            .iter()
            .any(|value| value == &warning.message)
        {
            project.warnings.push(warning.message.clone());
        }
    }
    let status = if host_warnings.is_empty() {
        "unverified"
    } else {
        "risk"
    };
    project.host_compatibility_warnings = host_warnings.clone();
    project.office_compatibility = OfficeCompatibilityReport {
        office_load_verified: false,
        status: status.to_string(),
        risks: host_warnings,
        notes: vec![
            "Package validation and source readback do not prove that Microsoft Office will load this VBA project without repair."
                .to_string(),
        ],
    };
}

fn host_compatibility_warnings(project: &SourceProject) -> Vec<HostCompatibilityWarning> {
    if project.family.trim().is_empty() {
        return Vec::new();
    }
    let mut excel_doc_modules = Vec::new();
    let mut powerpoint_doc_modules = Vec::new();
    for module in &project.modules {
        if !module_is_document_like(module) {
            continue;
        }
        let name = module.name.trim();
        if is_excel_document_module_name(name) {
            excel_doc_modules.push(name.to_string());
        } else if is_powerpoint_document_module_name(name) {
            powerpoint_doc_modules.push(name.to_string());
        }
    }
    let mut warnings = Vec::new();
    if project.family == "pptx" && !excel_doc_modules.is_empty() {
        warnings.push(HostCompatibilityWarning {
            code: "VBA_HOST_EXCEL_MODULES_IN_PPTM".to_string(),
            message: format!(
                "PowerPoint macro package contains Excel document module(s): {}. The package can be structurally valid while Office may repair or reject the VBA project; use a PowerPoint-native vbaProject.bin seed for PPTM outputs.",
                excel_doc_modules.join(", ")
            ),
            modules: excel_doc_modules,
        });
    }
    if project.family == "xlsx" && !powerpoint_doc_modules.is_empty() {
        warnings.push(HostCompatibilityWarning {
            code: "VBA_HOST_POWERPOINT_MODULES_IN_XLSM".to_string(),
            message: format!(
                "Excel macro package contains PowerPoint document-like module(s): {}. The package can be structurally valid while Office may repair or reject the VBA project; use an Excel-native vbaProject.bin seed for XLSM outputs.",
                powerpoint_doc_modules.join(", ")
            ),
            modules: powerpoint_doc_modules,
        });
    }
    warnings
}

fn module_is_document_like(module: &SourceModule) -> bool {
    module.kind.eq_ignore_ascii_case("class") || module.extension.eq_ignore_ascii_case(".cls")
}

fn is_excel_document_module_name(name: &str) -> bool {
    let normalized = name.trim().to_ascii_lowercase();
    normalized == "thisworkbook"
        || normalized
            .strip_prefix("sheet")
            .is_some_and(all_ascii_digits)
        || normalized
            .strip_prefix("chart")
            .is_some_and(all_ascii_digits)
}

fn is_powerpoint_document_module_name(name: &str) -> bool {
    let normalized = name.trim().to_ascii_lowercase();
    normalized == "thispresentation"
        || normalized
            .strip_prefix("slide")
            .is_some_and(all_ascii_digits)
}

fn all_ascii_digits(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit())
}

fn find_cfb_stream_path(paths: &[String], want: &str) -> String {
    paths
        .iter()
        .find(|path| path.replace('\\', "/").eq_ignore_ascii_case(want))
        .cloned()
        .unwrap_or_default()
}

fn source_project_json(project: &SourceProject, include_source: bool) -> Value {
    let mut object = Map::new();
    if !project.family.is_empty() {
        object.insert("family".to_string(), json!(project.family));
    }
    if !project.part_uri.is_empty() {
        object.insert("partUri".to_string(), json!(project.part_uri));
    }
    if project.code_page != 0 {
        object.insert("codePage".to_string(), json!(project.code_page));
    }
    object.insert("moduleCount".to_string(), json!(project.module_count));
    object.insert(
        "modules".to_string(),
        Value::Array(
            project
                .modules
                .iter()
                .map(|module| source_module_json(module, include_source))
                .collect(),
        ),
    );
    if let Some(metadata) = &project.project_metadata {
        object.insert(
            "projectMetadata".to_string(),
            project_metadata_json(metadata),
        );
    }
    object.insert(
        "officeCompatibility".to_string(),
        office_compatibility_json(&project.office_compatibility),
    );
    if !project.host_compatibility_warnings.is_empty() {
        object.insert(
            "hostCompatibilityWarnings".to_string(),
            Value::Array(
                project
                    .host_compatibility_warnings
                    .iter()
                    .map(host_compatibility_warning_json)
                    .collect(),
            ),
        );
    }
    if !project.warnings.is_empty() {
        object.insert("warnings".to_string(), json!(project.warnings));
    }
    Value::Object(object)
}

fn source_module_json(module: &SourceModule, include_source: bool) -> Value {
    let mut object = Map::new();
    object.insert("number".to_string(), json!(module.number));
    object.insert("name".to_string(), json!(module.name));
    object.insert("streamName".to_string(), json!(module.stream_name));
    object.insert("kind".to_string(), json!(module.kind));
    object.insert("extension".to_string(), json!(module.extension));
    if module.code_page != 0 {
        object.insert("codePage".to_string(), json!(module.code_page));
    }
    object.insert("sourceOffset".to_string(), json!(module.source_offset));
    if let Some(source_bytes) = module.source_bytes {
        object.insert("sourceBytes".to_string(), json!(source_bytes));
    }
    if let Some(line_count) = module.line_count {
        object.insert("lineCount".to_string(), json!(line_count));
    }
    if !module.sha256.is_empty() {
        object.insert("sha256".to_string(), json!(module.sha256));
    }
    if !module.sha256_basis.is_empty() {
        object.insert("sha256Basis".to_string(), json!(module.sha256_basis));
    }
    if !module.line_ending.is_empty() {
        object.insert("lineEnding".to_string(), json!(module.line_ending));
    }
    object.insert(
        "trailingNewline".to_string(),
        json!(module.trailing_newline),
    );
    if !module.primary_selector.is_empty() {
        object.insert(
            "primarySelector".to_string(),
            json!(module.primary_selector),
        );
    }
    if !module.selectors.is_empty() {
        object.insert("selectors".to_string(), json!(module.selectors));
    }
    if include_source && !module.source.is_empty() {
        object.insert("source".to_string(), json!(module.source));
    }
    if !module.warnings.is_empty() {
        object.insert("warnings".to_string(), json!(module.warnings));
    }
    Value::Object(object)
}

fn project_metadata_json(metadata: &ProjectMetadata) -> Value {
    let mut object = Map::new();
    if !metadata.stream_name.is_empty() {
        object.insert("streamName".to_string(), json!(metadata.stream_name));
    }
    object.insert("present".to_string(), json!(metadata.present));
    if metadata.line_count > 0 {
        object.insert("lineCount".to_string(), json!(metadata.line_count));
    }
    if !metadata.id.is_empty() {
        object.insert("id".to_string(), json!(metadata.id));
    }
    if !metadata.name.is_empty() {
        object.insert("name".to_string(), json!(metadata.name));
    }
    if !metadata.modules.is_empty() {
        object.insert(
            "modules".to_string(),
            Value::Array(
                metadata
                    .modules
                    .iter()
                    .map(project_module_declaration_json)
                    .collect(),
            ),
        );
    }
    if !metadata.references.is_empty() {
        object.insert(
            "references".to_string(),
            Value::Array(
                metadata
                    .references
                    .iter()
                    .map(project_reference_json)
                    .collect(),
            ),
        );
    }
    if !metadata.workspace_entries.is_empty() {
        object.insert(
            "workspaceEntries".to_string(),
            Value::Array(
                metadata
                    .workspace_entries
                    .iter()
                    .map(project_workspace_entry_json)
                    .collect(),
            ),
        );
    }
    if metadata.has_project_wm {
        object.insert("hasProjectWm".to_string(), json!(true));
    }
    if !metadata.project_wm_stream.is_empty() {
        object.insert(
            "projectWmStream".to_string(),
            json!(metadata.project_wm_stream),
        );
    }
    if !metadata.warnings.is_empty() {
        object.insert("warnings".to_string(), json!(metadata.warnings));
    }
    Value::Object(object)
}

fn project_module_declaration_json(module: &ProjectModuleDeclaration) -> Value {
    let mut object = Map::new();
    object.insert("kind".to_string(), json!(module.kind));
    object.insert("name".to_string(), json!(module.name));
    if !module.value.is_empty() {
        object.insert("value".to_string(), json!(module.value));
    }
    object.insert("line".to_string(), json!(module.line));
    Value::Object(object)
}

fn project_reference_json(reference: &ProjectReference) -> Value {
    json!({
        "kind": reference.kind,
        "value": reference.value,
        "line": reference.line,
    })
}

fn project_workspace_entry_json(entry: &ProjectWorkspaceEntry) -> Value {
    json!({
        "name": entry.name,
        "value": entry.value,
        "line": entry.line,
    })
}

fn office_compatibility_json(report: &OfficeCompatibilityReport) -> Value {
    let mut object = Map::new();
    object.insert(
        "officeLoadVerified".to_string(),
        json!(report.office_load_verified),
    );
    object.insert("status".to_string(), json!(report.status));
    if !report.risks.is_empty() {
        object.insert(
            "risks".to_string(),
            Value::Array(
                report
                    .risks
                    .iter()
                    .map(host_compatibility_warning_json)
                    .collect(),
            ),
        );
    }
    if !report.notes.is_empty() {
        object.insert("notes".to_string(), json!(report.notes));
    }
    Value::Object(object)
}

fn host_compatibility_warning_json(warning: &HostCompatibilityWarning) -> Value {
    let mut object = Map::new();
    object.insert("code".to_string(), json!(warning.code));
    object.insert("message".to_string(), json!(warning.message));
    if !warning.modules.is_empty() {
        object.insert("modules".to_string(), json!(warning.modules));
    }
    Value::Object(object)
}

fn module_extract_item_json(
    module: &SourceModule,
    output_path: &Path,
    bytes_written: usize,
) -> Value {
    let mut object = Map::new();
    object.insert("number".to_string(), json!(module.number));
    object.insert("name".to_string(), json!(module.name));
    object.insert("streamName".to_string(), json!(module.stream_name));
    object.insert("kind".to_string(), json!(module.kind));
    object.insert("extension".to_string(), json!(module.extension));
    object.insert(
        "outputPath".to_string(),
        json!(output_path.to_string_lossy().to_string()),
    );
    object.insert("bytesWritten".to_string(), json!(bytes_written));
    if let Some(line_count) = module.line_count {
        object.insert("lineCount".to_string(), json!(line_count));
    }
    if !module.sha256.is_empty() {
        object.insert("sha256".to_string(), json!(module.sha256));
    }
    if !module.sha256_basis.is_empty() {
        object.insert("sha256Basis".to_string(), json!(module.sha256_basis));
    }
    if !module.line_ending.is_empty() {
        object.insert("lineEnding".to_string(), json!(module.line_ending));
    }
    object.insert(
        "trailingNewline".to_string(),
        json!(module.trailing_newline),
    );
    if !module.primary_selector.is_empty() {
        object.insert(
            "primarySelector".to_string(),
            json!(module.primary_selector),
        );
    }
    if !module.selectors.is_empty() {
        object.insert("selectors".to_string(), json!(module.selectors));
    }
    if !module.warnings.is_empty() {
        object.insert("warnings".to_string(), json!(module.warnings));
    }
    Value::Object(object)
}

fn read_u16(data: &[u8], offset: usize) -> Result<u16, String> {
    let bytes = data
        .get(offset..offset + 2)
        .ok_or_else(|| "truncated VBA dir stream".to_string())?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn read_u32(data: &[u8], offset: usize) -> Result<u32, String> {
    let bytes = data
        .get(offset..offset + 4)
        .ok_or_else(|| "truncated VBA dir stream".to_string())?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}
