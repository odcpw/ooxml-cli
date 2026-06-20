use std::collections::BTreeMap;

use crate::{CliError, CliResult};

use super::super::cfb::{
    CfbFile, rewrite_streams_with_adds_and_deletes, rewrite_streams_with_deletes,
};
use super::cfb_paths::optional_cfb_stream_path;
use super::codec::{
    compress_container_literals, decompress_container, encode_module_source,
    extension_for_module_kind, source_sha256,
};
use super::dir::{add_dir_module, remove_dir_module, rewrite_dir_module_offset};
use super::parser::{map_source_parse_error, parse_source_project};
use super::project_metadata::{
    add_project_stream_module_lines, add_project_wm_module_entry,
    remove_project_stream_module_lines, remove_project_wm_module_entry,
};
use super::selectors::{select_modules, with_source_module_selectors};
use super::{DIR_STREAM_PATH, SourceModule, SourceProject};

pub(super) struct SourceMutationOptions {
    pub(super) allow_experimental_source_rewrite: bool,
    pub(super) source_kind: String,
}

pub(super) struct AddModuleOptions {
    pub(super) name: String,
    pub(super) kind: String,
    pub(super) expect_module_count: Option<usize>,
    pub(super) allow_experimental_source_rewrite: bool,
}

pub(super) struct SourceMutationResult {
    pub(super) action: String,
    pub(super) family: String,
    pub(super) part_uri: String,
    pub(super) module: SourceModule,
    pub(super) previous_count: Option<usize>,
    pub(super) module_count: Option<usize>,
    pub(super) previous_sha256: String,
    pub(super) sha256: String,
    pub(super) source_bytes: Option<usize>,
    pub(super) line_count: Option<usize>,
    pub(super) warnings: Vec<String>,
    pub(super) purged_caches: bool,
    pub(super) recompiles_on_open: bool,
    pub(super) office_load_verified: bool,
    pub(super) compatibility_status: String,
}
pub(super) fn replace_module_source_in_project_data(
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

pub(super) fn add_module_source_in_project_data(
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

pub(super) fn remove_module_source_in_project_data(
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
