mod codec;
mod forms;
mod model;
mod records;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use crate::{CliError, CliResult, command_arg};

use model::{VbaModuleKind, VbaModuleModel, VbaProjectModel, VbaUserFormModel};

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
    for module in &project.modules {
        if module.kind == VbaModuleKind::UserForm {
            rendered
                .streams
                .extend(forms::render_user_form_storage_streams(module));
        }
    }
    Ok(rendered.streams)
}

fn build_vba_project_bin(project: &VbaProjectModel) -> VbaAuthoringResult<Vec<u8>> {
    let streams = render_complete_stream_map(project)?;
    build_streams_file(&streams).map_err(VbaAuthoringError::build_failed)
}

pub(crate) fn vba_xlsx_standard_module_project_bin(
    module_name: &str,
    source: &str,
    sheet_code_names: &[&str],
) -> CliResult<Vec<u8>> {
    let mut modules = Vec::new();
    modules.push(VbaModuleModel::excel_workbook_document());
    for sheet_code_name in sheet_code_names {
        modules.push(VbaModuleModel::excel_sheet_document(*sheet_code_name));
    }
    modules.push(VbaModuleModel::new(
        module_name,
        None::<String>,
        VbaModuleKind::Standard,
        source.as_bytes().to_vec(),
    ));
    let project = VbaProjectModel::xlsx(modules);
    build_vba_project_bin(&project).map_err(authoring_error_to_cli)
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
            "--family is required when vba create --pure input extension is not .xlsx, .xlsm, .pptx, .pptm, .docx, or .docm",
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
        "docx" | "docm" => Some("docx".to_string()),
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
            "no .bas, .cls, .frm, or .frx files found under --source-dir {source_dir}"
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
        "bas" | "cls" | "frm" | "frx"
    )
}

fn build_bin_from_sources(family: Option<&str>, sources: &[String]) -> CliResult<BuildBinOutcome> {
    let family = normalize_build_family(family.unwrap_or_default())?;
    let source_paths = normalize_source_values(sources)?;
    let source_modules = read_source_modules(&source_paths)?;
    reject_unsupported_userform_family(&family, &source_modules)?;
    let inserted_vb_names = source_modules
        .iter()
        .filter(|input| input.inserted_vb_name)
        .map(|input| input.module.name.clone())
        .collect::<Vec<_>>();
    let mut user_modules = source_modules
        .iter()
        .map(|input| input.module.clone())
        .collect::<Vec<_>>();
    if family == "xlsx" && needs_excel_host_document_modules(&user_modules) {
        user_modules = with_excel_host_document_modules(user_modules);
    }
    if family == "docx" {
        user_modules = with_word_host_document_module(user_modules);
    }
    let project = match family.as_str() {
        "xlsx" => VbaProjectModel::xlsx(user_modules),
        "pptx" => VbaProjectModel::pptx(user_modules),
        "docx" => VbaProjectModel::docx(user_modules),
        _ => {
            return Err(CliError::unsupported_type(
                "pure VBA authoring supports only --family xlsx, pptx, or docx",
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

fn reject_unsupported_userform_family(
    family: &str,
    source_modules: &[SourceModuleInput],
) -> CliResult<()> {
    if family == "xlsx"
        || !source_modules
            .iter()
            .any(|input| input.module.kind == VbaModuleKind::UserForm)
    {
        return Ok(());
    }
    Err(CliError::unsupported_type(
        "pure UserForm authoring is currently supported only for --family xlsx; PPTM/DOCM UserForm packaging is not Office-proven yet",
    ))
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

fn needs_excel_host_document_modules(modules: &[VbaModuleModel]) -> bool {
    modules
        .iter()
        .any(|module| matches!(module.kind, VbaModuleKind::Class | VbaModuleKind::UserForm))
}

fn with_excel_host_document_modules(mut user_modules: Vec<VbaModuleModel>) -> Vec<VbaModuleModel> {
    let mut modules = vec![
        VbaModuleModel::excel_workbook_document(),
        VbaModuleModel::excel_sheet_document("Sheet1"),
    ];
    modules.append(&mut user_modules);
    modules
}

fn with_word_host_document_module(mut user_modules: Vec<VbaModuleModel>) -> Vec<VbaModuleModel> {
    let mut modules = vec![VbaModuleModel::word_document_document()];
    modules.append(&mut user_modules);
    modules
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
        "docx" => format!(
            "ooxml --json vba attach document.docx --bin {} --out document.docm",
            command_arg(bin_path)
        ),
        _ => format!(
            "ooxml --json vba attach <target.pptx|target.docx|target.xlsx> --bin {} --out <macro-output.pptm|macro-output.docm|macro-output.xlsm>",
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
            "--source is required (repeat it for each .bas/.cls/.frm file)",
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
    let raw_text = String::from_utf8_lossy(&data);
    let normalized_raw_text = codec::normalize_vba_line_endings(&raw_text);
    let user_form_caption = (kind == VbaModuleKind::UserForm)
        .then(|| userform_caption_from_export(&normalized_raw_text))
        .flatten();
    let text = normalized_source_text_for_kind(kind, &raw_text);
    let attr_name = vb_name_attribute(&text);
    let name = attr_name.unwrap_or_else(|| {
        Path::new(path)
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("Module1")
            .to_string()
    });
    let inserted_vb_name = vb_name_attribute(&text).is_none();
    let mut source_text = if inserted_vb_name {
        format!("Attribute VB_Name = \"{name}\"\r\n{text}").into_bytes()
    } else {
        text.into_bytes()
    };
    if kind == VbaModuleKind::Class {
        source_text =
            ensure_class_module_attributes(&String::from_utf8_lossy(&source_text)).into_bytes();
    }
    if kind == VbaModuleKind::UserForm {
        source_text =
            ensure_userform_module_attributes(&String::from_utf8_lossy(&source_text)).into_bytes();
    }
    let module = if kind == VbaModuleKind::UserForm {
        VbaModuleModel::user_form(
            name.clone(),
            None::<String>,
            source_text,
            VbaUserFormModel::new(user_form_caption.unwrap_or_else(|| name.clone())),
        )
    } else {
        VbaModuleModel::new(name, None::<String>, kind, source_text)
    };
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
        "frm" => Ok(VbaModuleKind::UserForm),
        "frx" => Err(CliError::unsupported_type(format!(
            "VBA .frx sidecars are not supported by pure UserForm authoring yet: {path}; pass the .frm only for a generated blank designer storage"
        ))),
        _ => Err(CliError::invalid_args(format!(
            "VBA source must be .bas, .cls, or .frm: {path}"
        ))),
    }
}

fn normalized_source_text_for_kind(kind: VbaModuleKind, text: &str) -> String {
    let normalized = codec::normalize_vba_line_endings(text);
    match kind {
        VbaModuleKind::Class => strip_exported_class_wrapper(&normalized),
        VbaModuleKind::UserForm => strip_exported_userform_wrapper(&normalized),
        _ => normalized,
    }
}

fn strip_exported_class_wrapper(text: &str) -> String {
    let lines = text.split("\r\n").collect::<Vec<_>>();
    if !lines
        .first()
        .is_some_and(|line| line.trim().eq_ignore_ascii_case("VERSION 1.0 CLASS"))
    {
        return text.to_string();
    }
    let Some(begin_idx) = lines
        .iter()
        .position(|line| line.trim().eq_ignore_ascii_case("BEGIN"))
    else {
        return text.to_string();
    };
    let Some(end_offset) = lines
        .iter()
        .skip(begin_idx + 1)
        .position(|line| line.trim().eq_ignore_ascii_case("END"))
    else {
        return text.to_string();
    };
    lines[begin_idx + 1 + end_offset + 1..].join("\r\n")
}

fn ensure_class_module_attributes(text: &str) -> String {
    let mut lines = text
        .split("\r\n")
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    if lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    let insert_at = lines
        .iter()
        .position(|line| {
            !line
                .trim_start()
                .to_ascii_lowercase()
                .starts_with("attribute ")
        })
        .unwrap_or(lines.len());
    let required = [
        (
            "vb_base",
            "Attribute VB_Base = \"0{FCFB3D2A-A0FA-1068-A738-08002B3371B5}\"",
        ),
        ("vb_globalnamespace", "Attribute VB_GlobalNameSpace = False"),
        ("vb_creatable", "Attribute VB_Creatable = False"),
        ("vb_predeclaredid", "Attribute VB_PredeclaredId = False"),
        ("vb_exposed", "Attribute VB_Exposed = False"),
        ("vb_templatederived", "Attribute VB_TemplateDerived = False"),
        ("vb_customizable", "Attribute VB_Customizable = False"),
    ];
    let mut missing = required
        .into_iter()
        .filter_map(|(key, line)| (!has_attribute(&lines, key)).then_some(line.to_string()))
        .collect::<Vec<_>>();
    if missing.is_empty() {
        let mut out = lines.join("\r\n");
        out.push_str("\r\n");
        return out;
    }
    for line in missing.drain(..).rev() {
        lines.insert(insert_at, line);
    }
    let mut out = lines.join("\r\n");
    out.push_str("\r\n");
    out
}

fn strip_exported_userform_wrapper(text: &str) -> String {
    let lines = text.split("\r\n").collect::<Vec<_>>();
    if !lines
        .first()
        .is_some_and(|line| line.trim().eq_ignore_ascii_case("VERSION 5.00"))
    {
        return text.to_string();
    }
    let mut depth = 0_usize;
    let mut end_idx = None;
    for (idx, line) in lines.iter().enumerate().skip(1) {
        let trimmed = line.trim_start();
        if trimmed.starts_with("Begin ") {
            depth += 1;
            continue;
        }
        if trimmed.eq_ignore_ascii_case("End") && depth > 0 {
            depth -= 1;
            if depth == 0 {
                end_idx = Some(idx);
                break;
            }
        }
    }
    let Some(end_idx) = end_idx else {
        return text.to_string();
    };
    lines[end_idx + 1..].join("\r\n")
}

fn ensure_userform_module_attributes(text: &str) -> String {
    let mut lines = text
        .split("\r\n")
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    if lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    let insert_at = lines
        .iter()
        .position(|line| {
            !line
                .trim_start()
                .to_ascii_lowercase()
                .starts_with("attribute ")
        })
        .unwrap_or(lines.len());
    let required = [
        (
            "vb_base",
            "Attribute VB_Base = \"0{F8A47041-B2A6-11CE-8027-00AA00611080}\"",
        ),
        ("vb_globalnamespace", "Attribute VB_GlobalNameSpace = False"),
        ("vb_creatable", "Attribute VB_Creatable = False"),
        ("vb_predeclaredid", "Attribute VB_PredeclaredId = True"),
        ("vb_exposed", "Attribute VB_Exposed = False"),
        ("vb_templatederived", "Attribute VB_TemplateDerived = False"),
        ("vb_customizable", "Attribute VB_Customizable = False"),
    ];
    let mut missing = required
        .into_iter()
        .filter_map(|(key, line)| (!has_attribute(&lines, key)).then_some(line.to_string()))
        .collect::<Vec<_>>();
    if missing.is_empty() {
        let mut out = lines.join("\r\n");
        out.push_str("\r\n");
        return out;
    }
    for line in missing.drain(..).rev() {
        lines.insert(insert_at, line);
    }
    let mut out = lines.join("\r\n");
    out.push_str("\r\n");
    out
}

fn userform_caption_from_export(text: &str) -> Option<String> {
    text.replace("\r\n", "\n")
        .replace('\r', "\n")
        .lines()
        .find_map(|line| {
            let trimmed = line.trim();
            let lower = trimmed.to_ascii_lowercase();
            if !lower.starts_with("caption") {
                return None;
            }
            let (_, value) = trimmed.split_once('=')?;
            let value = value.trim().trim_matches('"').trim();
            (!value.is_empty()).then(|| value.to_string())
        })
}

fn has_attribute(lines: &[String], name: &str) -> bool {
    lines.iter().any(|line| {
        let trimmed = line.trim();
        let lower = trimmed.to_ascii_lowercase();
        lower
            .strip_prefix("attribute ")
            .and_then(|rest| rest.split_once('='))
            .is_some_and(|(candidate, _)| candidate.trim() == name)
    })
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

    #[test]
    fn imported_class_source_strips_office_export_wrapper() {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let temp_dir = std::env::temp_dir().join(format!(
            "ooxml-vba-class-source-{}-{suffix}",
            std::process::id()
        ));
        fs::create_dir_all(&temp_dir).expect("temp dir");
        let path = temp_dir.join("Worker.cls");
        fs::write(
            &path,
            "VERSION 1.0 CLASS\r\nBEGIN\r\n  MultiUse = -1\r\nEND\r\nAttribute VB_Name = \"Worker\"\r\nPublic Function Answer()\r\nAnswer = 42\r\nEnd Function\r\n",
        )
        .expect("write class");

        let input = read_source_module(&path.to_string_lossy()).expect("read source module");
        let source = String::from_utf8(input.module.source).expect("source utf8");

        assert_eq!(input.module.kind, VbaModuleKind::Class);
        assert_eq!(input.module.name, "Worker");
        assert!(source.starts_with("Attribute VB_Name = \"Worker\"\r\n"));
        assert!(source.contains("Public Function Answer()"));
        assert!(!source.contains("VERSION 1.0 CLASS"));
        assert!(!source.contains("MultiUse = -1"));
        assert!(source.contains("Attribute VB_Base = \"0{FCFB3D2A-A0FA-1068-A738-08002B3371B5}\""));
        assert!(source.contains("Attribute VB_TemplateDerived = False"));
        assert!(source.contains("Attribute VB_Customizable = False"));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn imported_userform_source_strips_designer_wrapper_and_adds_attributes() {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let temp_dir = std::env::temp_dir().join(format!(
            "ooxml-vba-userform-source-{}-{suffix}",
            std::process::id()
        ));
        fs::create_dir_all(&temp_dir).expect("temp dir");
        let path = temp_dir.join("Dialog.frm");
        fs::write(
            &path,
            "VERSION 5.00\r\nBegin {C62A69F0-16DC-11CE-9E98-00AA00574A4F} Dialog \r\n   Caption         =   \"Agent Dialog\"\r\nEnd\r\nAttribute VB_Name = \"Dialog\"\r\nPrivate Sub UserForm_Initialize()\r\nEnd Sub\r\n",
        )
        .expect("write form");

        let input = read_source_module(&path.to_string_lossy()).expect("read userform module");
        let source = String::from_utf8(input.module.source).expect("source utf8");

        assert_eq!(input.module.kind, VbaModuleKind::UserForm);
        assert_eq!(input.module.name, "Dialog");
        assert_eq!(
            input.module.user_form.as_ref().unwrap().caption,
            "Agent Dialog"
        );
        assert!(source.starts_with("Attribute VB_Name = \"Dialog\"\r\n"));
        assert!(source.contains("Private Sub UserForm_Initialize()"));
        assert!(!source.contains("VERSION 5.00"));
        assert!(source.contains("Attribute VB_Base = \"0{F8A47041-B2A6-11CE-8027-00AA00611080}\""));
        assert!(source.contains("Attribute VB_PredeclaredId = True"));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn frx_sidecars_are_refused_by_pure_authoring() {
        let err = source_kind_from_path("Dialog.frx").expect_err("frx refusal");
        assert_eq!(err.code, "unsupported_type");
        assert!(
            err.message.contains(".frx sidecars are not supported"),
            "unexpected .frx error: {err:?}"
        );
    }

    #[test]
    fn xlsx_class_authoring_synthesizes_excel_host_documents() {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let temp_dir = std::env::temp_dir().join(format!(
            "ooxml-vba-class-host-docs-{}-{suffix}",
            std::process::id()
        ));
        fs::create_dir_all(&temp_dir).expect("temp dir");
        let worker_path = temp_dir.join("Worker.cls");
        fs::write(
            &worker_path,
            "Attribute VB_Name = \"Worker\"\r\nPublic Function Answer()\r\nAnswer = 42\r\nEnd Function\r\n",
        )
        .expect("write class source");

        let project =
            build_bin_from_sources(Some("xlsx"), &[worker_path.to_string_lossy().to_string()])
                .expect("build class project");

        assert_eq!(project.project.modules.len(), 3);
        assert_eq!(project.project.modules[0].name, "ThisWorkbook");
        assert_eq!(project.project.modules[0].kind, VbaModuleKind::Document);
        assert_eq!(project.project.modules[1].name, "Sheet1");
        assert_eq!(project.project.modules[1].kind, VbaModuleKind::Document);
        assert_eq!(project.project.modules[2].name, "Worker");
        assert_eq!(project.project.modules[2].kind, VbaModuleKind::Class);
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn xlsx_userform_authoring_synthesizes_excel_host_documents() {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let temp_dir = std::env::temp_dir().join(format!(
            "ooxml-vba-userform-host-docs-{}-{suffix}",
            std::process::id()
        ));
        fs::create_dir_all(&temp_dir).expect("temp dir");
        let form_path = temp_dir.join("Dialog.frm");
        fs::write(
            &form_path,
            "VERSION 5.00\r\nBegin {C62A69F0-16DC-11CE-9E98-00AA00574A4F} Dialog \r\nEnd\r\nAttribute VB_Name = \"Dialog\"\r\nPrivate Sub UserForm_Initialize()\r\nEnd Sub\r\n",
        )
        .expect("write form source");

        let project =
            build_bin_from_sources(Some("xlsx"), &[form_path.to_string_lossy().to_string()])
                .expect("build userform project");

        assert_eq!(project.project.modules.len(), 3);
        assert_eq!(project.project.modules[0].name, "ThisWorkbook");
        assert_eq!(project.project.modules[0].kind, VbaModuleKind::Document);
        assert_eq!(project.project.modules[1].name, "Sheet1");
        assert_eq!(project.project.modules[1].kind, VbaModuleKind::Document);
        assert_eq!(project.project.modules[2].name, "Dialog");
        assert_eq!(project.project.modules[2].kind, VbaModuleKind::UserForm);
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn docx_authoring_synthesizes_word_host_document() {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let temp_dir = std::env::temp_dir().join(format!(
            "ooxml-vba-word-host-doc-{}-{suffix}",
            std::process::id()
        ));
        fs::create_dir_all(&temp_dir).expect("temp dir");
        let module_path = temp_dir.join("AgentDoc.bas");
        fs::write(
            &module_path,
            "Attribute VB_Name = \"AgentDoc\"\r\nPublic Sub MarkDocument()\r\nEnd Sub\r\n",
        )
        .expect("write module source");

        let project =
            build_bin_from_sources(Some("docx"), &[module_path.to_string_lossy().to_string()])
                .expect("build docx project");

        assert_eq!(project.project.project_name, "Project");
        assert_eq!(project.project.modules.len(), 2);
        assert_eq!(project.project.modules[0].name, "ThisDocument");
        assert_eq!(project.project.modules[0].kind, VbaModuleKind::Document);
        assert_eq!(project.project.modules[1].name, "AgentDoc");
        assert_eq!(project.project.modules[1].kind, VbaModuleKind::Standard);
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
