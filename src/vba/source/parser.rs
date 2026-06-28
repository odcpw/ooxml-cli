use std::collections::BTreeSet;

use sha2::{Digest, Sha256};

use crate::{CliError, CliResult, zip_bytes};

use super::super::cfb::CfbFile;
use super::super::inspect::inspect_vba_package;
use super::super::model::VbaInfo;
use super::super::output::{vba_inspect_command, vba_next_mutation_template};
use super::super::package_xml::package_part_name;
use super::codec::{
    count_source_lines, decode_module_source, decompress_container, extension_for_module_kind,
    source_has_trailing_line_ending, source_line_ending_style,
};
use super::compatibility::populate_office_compatibility;
use super::dir::parse_dir_stream;
use super::project_metadata::parse_project_metadata;
use super::selectors::with_source_module_selectors;
use super::{DIR_STREAM_PATH, OfficeCompatibilityReport, SourceModule, SourceProject};
pub(super) fn inspect_source_project_for_file(file: &str) -> CliResult<(VbaInfo, SourceProject)> {
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

pub(super) fn parse_source_project_for_family(
    data: &[u8],
    family: &str,
) -> CliResult<SourceProject> {
    let mut project = parse_source_project(data).map_err(map_source_parse_error)?;
    project.family = family.to_ascii_lowercase();
    populate_office_compatibility(&mut project);
    Ok(project)
}

pub(super) fn normalize_inspect_bin_family(value: &str) -> CliResult<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "" => Err(CliError::invalid_args(
            "--family is required for inspect-bin (pptx, docx, or xlsx)",
        )),
        "pptx" | "pptm" | "powerpoint" | "presentation" => Ok("pptx".to_string()),
        "docx" | "docm" | "word" | "document" => Ok("docx".to_string()),
        "xlsx" | "xlsm" | "excel" | "workbook" => Ok("xlsx".to_string()),
        _ => Err(CliError::invalid_args(
            "--family must be pptx, docx, or xlsx",
        )),
    }
}

pub(super) fn map_source_parse_error(message: String) -> CliError {
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

pub(super) fn parse_source_project(data: &[u8]) -> Result<SourceProject, String> {
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

    let user_form_names = user_form_names_from_project_metadata(&project);

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
        if item.name.is_empty() {
            item.name.clone_from(&item.stream_name);
        }
        if item.kind.is_empty() {
            item.kind = "unknown".to_string();
            item.extension = ".bas".to_string();
            item.warnings
                .push("module type was not present in dir stream".to_string());
        } else if user_form_names.contains(&item.name.to_ascii_lowercase())
            || user_form_names.contains(&item.stream_name.to_ascii_lowercase())
        {
            item.kind = "userform".to_string();
        }
        item.extension = extension_for_module_kind(&item.kind).to_string();
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

fn user_form_names_from_project_metadata(project: &SourceProject) -> BTreeSet<String> {
    project
        .project_metadata
        .as_ref()
        .map(|metadata| {
            metadata
                .modules
                .iter()
                .filter(|module| module.kind.eq_ignore_ascii_case("baseclass"))
                .flat_map(|module| {
                    [
                        module.name.trim().to_ascii_lowercase(),
                        module.value.trim().to_ascii_lowercase(),
                    ]
                })
                .filter(|name| !name.is_empty())
                .collect()
        })
        .unwrap_or_default()
}
