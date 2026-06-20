use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::{CliError, CliResult, selector_candidates, zip_bytes};

use super::cfb::CfbFile;
use super::inspect::inspect_vba_package;
use super::model::VbaInfo;
use super::output::{
    vba_extract_modules_template, vba_info_json, vba_inspect_command, vba_list_command,
    vba_next_mutation_template, vba_office_check_command, vba_package_readback_command,
    vba_standalone_attach_template, vba_validate_command,
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
    count: usize,
    modules_start: usize,
}

struct DirReader<'a> {
    data: &'a [u8],
    pos: usize,
    code_page: i32,
    modules: Vec<DirModule>,
    warnings: Vec<String>,
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
                count,
                modules_start,
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
