mod codec;
mod model;
mod records;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use crate::{CliError, CliResult, command_arg};

use model::{VbaModuleKind, VbaModuleModel, VbaProjectModel};

use super::cfb::build_streams_file;
use super::model::VbaMutationOptions;
use super::mutation::attach_vba_project_bytes;

type VbaStreamMap = BTreeMap<String, Vec<u8>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VbaAuthoringErrorKind {
    InvalidModel,
    BuildFailed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct VbaAuthoringError {
    kind: VbaAuthoringErrorKind,
    message: String,
}

impl VbaAuthoringError {
    fn invalid_model(message: impl Into<String>) -> Self {
        Self {
            kind: VbaAuthoringErrorKind::InvalidModel,
            message: message.into(),
        }
    }

    fn build_failed(message: impl Into<String>) -> Self {
        Self {
            kind: VbaAuthoringErrorKind::BuildFailed,
            message: message.into(),
        }
    }
}

type VbaAuthoringResult<T> = Result<T, VbaAuthoringError>;

#[derive(Debug, Clone, PartialEq, Eq)]
struct RenderedVbaStreams {
    streams: VbaStreamMap,
    warnings: Vec<String>,
}

fn render_known_streams(project: &VbaProjectModel) -> VbaAuthoringResult<RenderedVbaStreams> {
    project.validate()?;

    let mut streams = BTreeMap::new();
    let mut warnings = Vec::new();
    streams.insert(
        "PROJECT".to_string(),
        records::render_project_stream(project),
    );
    streams.insert(
        "PROJECTwm".to_string(),
        records::render_project_wm_stream(project),
    );
    streams.insert(
        "VBA/dir".to_string(),
        codec::compress_container_literals(&records::render_dir_stream(project)),
    );

    for module in &project.modules {
        let (encoded_source, mut module_warnings) =
            codec::encode_module_source(&module.source, project.code_page)?;
        warnings.append(&mut module_warnings);
        streams.insert(
            format!("VBA/{}", module.stream_name),
            codec::compress_container_literals(&encoded_source),
        );
    }

    Ok(RenderedVbaStreams { streams, warnings })
}

fn render_complete_stream_map(project: &VbaProjectModel) -> VbaAuthoringResult<VbaStreamMap> {
    let mut rendered = render_known_streams(project)?;
    rendered.streams.insert(
        "VBA/_VBA_PROJECT".to_string(),
        records::render_vba_project_stream(),
    );
    Ok(rendered.streams)
}

fn build_vba_project_bin(project: &VbaProjectModel) -> VbaAuthoringResult<Vec<u8>> {
    let streams = render_complete_stream_map(project)?;
    build_streams_file(&streams).map_err(VbaAuthoringError::build_failed)
}

pub(crate) struct VbaBuildBinOptions<'a> {
    pub(crate) family: Option<&'a str>,
    pub(crate) sources: Vec<String>,
    pub(crate) out: &'a str,
    pub(crate) force: bool,
}

pub(crate) struct VbaPureCreateOptions<'a> {
    pub(crate) family: Option<&'a str>,
    pub(crate) sources: Vec<String>,
    pub(crate) mutation: VbaMutationOptions<'a>,
}

pub(crate) struct VbaRebuildOptions<'a> {
    pub(crate) family: Option<&'a str>,
    pub(crate) source_dir: &'a str,
    pub(crate) mutation: VbaMutationOptions<'a>,
}

struct SourceModuleInput {
    path: String,
    module: VbaModuleModel,
    inserted_vb_name: bool,
}

struct BuildBinOutcome {
    family: String,
    project: VbaProjectModel,
    source_modules: Vec<SourceModuleInput>,
    inserted_vb_names: Vec<String>,
    bin: Vec<u8>,
    sha256: String,
}

pub(crate) fn vba_build_bin(options: VbaBuildBinOptions<'_>) -> CliResult<Value> {
    let out = options.out.trim();
    if out.is_empty() {
        return Err(CliError::invalid_args("--out is required"));
    }
    let outcome = build_bin_from_sources(options.family, &options.sources)?;

    let out_path = Path::new(out);
    if out_path.exists() && !options.force {
        return Err(CliError::invalid_args(format!(
            "output already exists: {out}; pass --force to overwrite"
        )));
    }
    if let Some(parent) = out_path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|err| {
            CliError::unexpected(format!("failed to create output directory: {err}"))
        })?;
    }
    fs::write(out_path, &outcome.bin)
        .map_err(|err| CliError::unexpected(format!("failed to write vbaProject.bin: {err}")))?;

    let mut result = Map::new();
    result.insert("family".to_string(), json!(outcome.family.clone()));
    result.insert("output".to_string(), json!(out));
    result.insert("bytesWritten".to_string(), json!(outcome.bin.len()));
    result.insert("sha256".to_string(), json!(outcome.sha256.clone()));
    result.insert("backend".to_string(), json!("pure-rust"));
    result.insert(
        "projectName".to_string(),
        json!(outcome.project.project_name.clone()),
    );
    result.insert("codePage".to_string(), json!(outcome.project.code_page));
    result.insert("modules".to_string(), module_summary_json(&outcome.project));
    result.insert(
        "sources".to_string(),
        source_summary_json(&outcome.source_modules),
    );
    result.insert("warnings".to_string(), json!(authoring_warnings(&outcome)));
    result.insert(
        "inspectBinCommand".to_string(),
        json!(format!(
            "ooxml --json vba inspect-bin {} --family {}",
            command_arg(out),
            outcome.family,
        )),
    );
    result.insert(
        "attachCommandTemplate".to_string(),
        json!(attach_command_template(&outcome.family, out)),
    );
    Ok(Value::Object(result))
}

pub(crate) fn vba_create_pure(file: &str, options: VbaPureCreateOptions<'_>) -> CliResult<Value> {
    let family = pure_create_family_from_input(file, options.family)?;
    let outcome = build_bin_from_sources(Some(&family), &options.sources)?;
    let mut result = attach_vba_project_bytes(file, outcome.bin.clone(), options.mutation)?;
    let Value::Object(ref mut map) = result else {
        return Ok(result);
    };
    map.insert("backend".to_string(), json!("pure-rust"));
    map.insert("createMode".to_string(), json!("pure"));
    map.insert(
        "authoring".to_string(),
        json!({
            "family": outcome.family.clone(),
            "projectName": outcome.project.project_name.clone(),
            "codePage": outcome.project.code_page,
            "bytesGenerated": outcome.bin.len(),
            "sha256": outcome.sha256.clone(),
            "modules": module_summary_json(&outcome.project),
            "sources": source_summary_json(&outcome.source_modules),
            "warnings": authoring_warnings(&outcome),
        }),
    );
    Ok(result)
}

pub(crate) fn vba_rebuild(file: &str, options: VbaRebuildOptions<'_>) -> CliResult<Value> {
    let source_paths = collect_source_dir_sources(options.source_dir)?;
    let family = pure_create_family_from_input(file, options.family)?;
    let outcome = build_bin_from_sources(Some(&family), &source_paths)?;
    let mut result = attach_vba_project_bytes(file, outcome.bin.clone(), options.mutation)?;
    let Value::Object(ref mut map) = result else {
        return Ok(result);
    };
    map.insert("backend".to_string(), json!("pure-rust"));
    map.insert("rebuildMode".to_string(), json!("pure"));
    map.insert("sourceDir".to_string(), json!(options.source_dir));
    map.insert("sourcesDiscovered".to_string(), json!(source_paths));
    map.insert(
        "authoring".to_string(),
        json!({
            "family": outcome.family.clone(),
            "projectName": outcome.project.project_name.clone(),
            "codePage": outcome.project.code_page,
            "bytesGenerated": outcome.bin.len(),
            "sha256": outcome.sha256.clone(),
            "modules": module_summary_json(&outcome.project),
            "sources": source_summary_json(&outcome.source_modules),
            "warnings": authoring_warnings(&outcome),
        }),
    );
    Ok(result)
}

fn pure_create_family_from_input(file: &str, family: Option<&str>) -> CliResult<String> {
    let explicit = family.unwrap_or_default().trim();
    if !explicit.is_empty() {
        let family = normalize_build_family(explicit)?;
        if let Some(input_family) = package_family_from_extension(file)
            && input_family != family
        {
            return Err(CliError::invalid_args(format!(
                "--family {family} does not match input package extension for {file}; expected --family {input_family}"
            )));
        }
        return Ok(family);
    }

    package_family_from_extension(file).ok_or_else(|| {
        CliError::invalid_args(
            "--family is required when vba create --pure input extension is not .xlsx, .xlsm, .pptx, or .pptm",
        )
    })
}

fn package_family_from_extension(file: &str) -> Option<String> {
    match Path::new(file)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "xlsx" | "xlsm" => Some("xlsx".to_string()),
        "pptx" | "pptm" => Some("pptx".to_string()),
        _ => None,
    }
}

fn collect_source_dir_sources(source_dir: &str) -> CliResult<Vec<String>> {
    let source_dir = source_dir.trim();
    if source_dir.is_empty() {
        return Err(CliError::invalid_args("--source-dir is required"));
    }
    let root = Path::new(source_dir);
    if !root.is_dir() {
        return Err(CliError::file_not_found(format!(
            "--source-dir must be an existing directory: {source_dir}"
        )));
    }

    let mut sources = Vec::new();
    collect_source_dir_sources_rec(root, &mut sources)?;
    sources.sort_by(|left, right| {
        left.to_string_lossy()
            .to_ascii_lowercase()
            .cmp(&right.to_string_lossy().to_ascii_lowercase())
            .then_with(|| left.cmp(right))
    });
    if sources.is_empty() {
        return Err(CliError::target_not_found(format!(
            "no .bas or .cls files found under --source-dir {source_dir}"
        )));
    }
    Ok(sources
        .into_iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect())
}

fn collect_source_dir_sources_rec(dir: &Path, sources: &mut Vec<PathBuf>) -> CliResult<()> {
    let entries = fs::read_dir(dir).map_err(|err| {
        CliError::file_not_found(format!(
            "failed to read source directory {}: {err}",
            dir.display()
        ))
    })?;
    for entry in entries {
        let entry = entry.map_err(|err| {
            CliError::file_not_found(format!(
                "failed to read source directory {}: {err}",
                dir.display()
            ))
        })?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|err| {
            CliError::file_not_found(format!(
                "failed to inspect source path {}: {err}",
                path.display()
            ))
        })?;
        if file_type.is_dir() {
            collect_source_dir_sources_rec(&path, sources)?;
        } else if file_type.is_file() && is_vba_source_path(&path) {
            sources.push(path);
        }
    }
    Ok(())
}

fn is_vba_source_path(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str(),
        "bas" | "cls"
    )
}

fn build_bin_from_sources(family: Option<&str>, sources: &[String]) -> CliResult<BuildBinOutcome> {
    let family = normalize_build_family(family.unwrap_or_default())?;
    if family == "docx" {
        return Err(CliError::unsupported_type(
            "pure VBA authoring does not support --family docx yet",
        ));
    }
    let source_paths = normalize_source_values(sources)?;
    let source_modules = read_source_modules(&source_paths)?;
    let inserted_vb_names = source_modules
        .iter()
        .filter(|input| input.inserted_vb_name)
        .map(|input| input.module.name.clone())
        .collect::<Vec<_>>();
    let user_modules = source_modules
        .iter()
        .map(|input| input.module.clone())
        .collect::<Vec<_>>();
    let project = match family.as_str() {
        "xlsx" => VbaProjectModel::xlsx(user_modules),
        "pptx" => VbaProjectModel::pptx(user_modules),
        _ => {
            return Err(CliError::unsupported_type(
                "pure VBA authoring supports only --family xlsx or pptx",
            ));
        }
    };
    let bin = build_vba_project_bin(&project).map_err(authoring_error_to_cli)?;
    let mut hasher = Sha256::new();
    hasher.update(&bin);
    let sha256 = format!("{:x}", hasher.finalize());
    Ok(BuildBinOutcome {
        family,
        project,
        source_modules,
        inserted_vb_names,
        bin,
        sha256,
    })
}

fn module_summary_json(project: &VbaProjectModel) -> Value {
    Value::Array(
        project
            .modules
            .iter()
            .map(|module| {
                json!({
                    "name": module.name.clone(),
                    "streamName": module.stream_name.clone(),
                    "kind": module.kind.as_str(),
                    "hostSynthesized": module.kind == VbaModuleKind::Document,
                })
            })
            .collect(),
    )
}

fn attach_command_template(family: &str, bin_path: &str) -> String {
    match family {
        "pptx" => format!(
            "ooxml --json vba attach deck.pptx --bin {} --out deck.pptm",
            command_arg(bin_path)
        ),
        "xlsx" => format!(
            "ooxml --json vba attach workbook.xlsx --bin {} --out workbook.xlsm",
            command_arg(bin_path)
        ),
        _ => format!(
            "ooxml --json vba attach <target.pptx|target.xlsx> --bin {} --out <macro-output.pptm|macro-output.xlsm>",
            command_arg(bin_path)
        ),
    }
}

fn source_summary_json(source_modules: &[SourceModuleInput]) -> Value {
    Value::Array(
        source_modules
            .iter()
            .map(|input| {
                json!({
                    "path": input.path.clone(),
                    "moduleName": input.module.name.clone(),
                    "kind": input.module.kind.as_str(),
                    "insertedVbNameAttribute": input.inserted_vb_name,
                })
            })
            .collect(),
    )
}

fn authoring_warnings(outcome: &BuildBinOutcome) -> Vec<String> {
    let mut warnings = Vec::new();
    if !outcome.inserted_vb_names.is_empty() {
        warnings.push(format!(
            "inserted Attribute VB_Name for module(s): {}",
            outcome.inserted_vb_names.join(", ")
        ));
    }
    warnings.push(
        "generated vbaProject.bin is source-only/cache-free; Office is expected to regenerate compiled cache streams on open"
            .to_string(),
    );
    warnings
}

fn normalize_build_family(value: &str) -> CliResult<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "" | "xlsx" | "xlsm" | "excel" | "workbook" => Ok("xlsx".to_string()),
        "pptx" | "pptm" | "powerpoint" | "presentation" => Ok("pptx".to_string()),
        "docx" | "docm" | "word" | "document" => Ok("docx".to_string()),
        _ => Err(CliError::invalid_args(
            "--family must be xlsx, pptx, or docx for pure VBA authoring",
        )),
    }
}

fn normalize_source_values(values: &[String]) -> CliResult<Vec<String>> {
    let mut out = Vec::new();
    for value in values {
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        if fs::metadata(value).is_ok() {
            out.push(value.to_string());
            continue;
        }
        out.extend(
            value
                .split(',')
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .map(ToOwned::to_owned),
        );
    }
    if out.is_empty() {
        return Err(CliError::invalid_args(
            "--source is required (repeat it for each .bas/.cls file)",
        ));
    }
    Ok(out)
}

fn read_source_modules(paths: &[String]) -> CliResult<Vec<SourceModuleInput>> {
    paths
        .iter()
        .map(|path| read_source_module(path))
        .collect::<CliResult<Vec<_>>>()
}

fn read_source_module(path: &str) -> CliResult<SourceModuleInput> {
    let data = fs::read(path).map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            CliError::file_not_found(format!("VBA source file not found: {path}"))
        } else {
            CliError::unexpected(format!("failed to read VBA source file {path}: {err}"))
        }
    })?;
    if data.is_empty() {
        return Err(CliError::invalid_args(format!(
            "VBA source file is empty: {path}"
        )));
    }
    let kind = source_kind_from_path(path)?;
    let text = String::from_utf8_lossy(&data);
    let attr_name = vb_name_attribute(&text);
    let name = attr_name.unwrap_or_else(|| {
        Path::new(path)
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("Module1")
            .to_string()
    });
    let inserted_vb_name = vb_name_attribute(&text).is_none();
    let source = if inserted_vb_name {
        let normalized = codec::normalize_vba_line_endings(&text);
        format!("Attribute VB_Name = \"{name}\"\r\n{normalized}").into_bytes()
    } else {
        data
    };
    let module = VbaModuleModel::new(name, None::<String>, kind, source);
    module
        .validate_for_build()
        .map_err(authoring_error_to_cli)?;
    Ok(SourceModuleInput {
        path: stable_path_display(path),
        module,
        inserted_vb_name,
    })
}

fn source_kind_from_path(path: &str) -> CliResult<VbaModuleKind> {
    match Path::new(path)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "bas" => Ok(VbaModuleKind::Standard),
        "cls" => Ok(VbaModuleKind::Class),
        _ => Err(CliError::invalid_args(format!(
            "VBA source must be .bas or .cls: {path}"
        ))),
    }
}

fn vb_name_attribute(text: &str) -> Option<String> {
    for line in text.replace("\r\n", "\n").replace('\r', "\n").lines() {
        let trimmed = line.trim();
        let lower = trimmed.to_ascii_lowercase();
        if !lower.starts_with("attribute vb_name") {
            continue;
        }
        let Some((_, value)) = trimmed.split_once('=') else {
            continue;
        };
        let value = value.trim().trim_matches('"').trim();
        if !value.is_empty() {
            return Some(value.to_string());
        }
    }
    None
}

fn stable_path_display(path: &str) -> String {
    PathBuf::from(path).to_string_lossy().to_string()
}

fn authoring_error_to_cli(error: VbaAuthoringError) -> CliError {
    match error.kind {
        VbaAuthoringErrorKind::InvalidModel => CliError::invalid_args(error.message),
        VbaAuthoringErrorKind::BuildFailed => CliError::unexpected(error.message),
    }
}

#[cfg(test)]
mod tests {
    use super::model::VbaModuleModel;
    use super::*;

    fn hello_project() -> VbaProjectModel {
        VbaProjectModel::xlsx(vec![VbaModuleModel::standard(
            "Module1",
            b"Attribute VB_Name = \"Module1\"\r\nPublic Sub Hello()\r\nEnd Sub\r\n".to_vec(),
        )])
    }

    #[test]
    fn renders_known_source_only_streams_without_claiming_complete_bin() {
        let rendered = render_known_streams(&hello_project()).expect("render known streams");
        let keys = rendered.streams.keys().cloned().collect::<Vec<_>>();
        assert_eq!(
            keys,
            vec![
                "PROJECT".to_string(),
                "PROJECTwm".to_string(),
                "VBA/Module1".to_string(),
                "VBA/dir".to_string(),
            ]
        );
        assert!(!rendered.streams.contains_key("VBA/_VBA_PROJECT"));

        let bin = build_vba_project_bin(&hello_project()).expect("complete CFB");
        assert!(bin.starts_with(&[0xD0, 0xCF, 0x11, 0xE0]));
    }

    #[test]
    fn rejects_invalid_model_before_build() {
        let project = VbaProjectModel::xlsx(Vec::new());
        let err = render_known_streams(&project).expect_err("invalid model");
        assert_eq!(err.kind, VbaAuthoringErrorKind::InvalidModel);
        assert!(err.message.contains("at least one"));
    }
}
