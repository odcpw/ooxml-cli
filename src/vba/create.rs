use serde_json::{Map, Value, json};
use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{CliError, CliResult, command_arg};

const OFFICE_CREATE_SCRIPT_DIR: &str = "tools";
const OFFICE_CREATE_SCRIPT_FILE: &str = "windows-office-vba-create.ps1";

pub(crate) struct VbaCreateOptions<'a> {
    pub(crate) family: Option<&'a str>,
    pub(crate) sources: Vec<String>,
    pub(crate) extract_bin: Option<&'a str>,
    pub(crate) office_create_script: Option<&'a str>,
    pub(crate) enable_vba_object_model_access: bool,
    pub(crate) visible: bool,
    pub(crate) force: bool,
}

struct ResolvedCreateOptions {
    family: String,
    output_path: String,
    source_paths: Vec<String>,
    extract_bin_path: String,
    office_create_script_path: String,
    enable_vba_object_model_access: bool,
    visible: bool,
    force: bool,
}

pub(crate) fn vba_create(output_path: &str, options: VbaCreateOptions<'_>) -> CliResult<Value> {
    let output_path = output_path.trim();
    if output_path.is_empty() {
        return Err(CliError::invalid_args("output path is required"));
    }
    let (family, _) = normalize_create_family(options.family.unwrap_or_default(), output_path)?;
    validate_create_output_extension(&family, output_path)?;
    let source_paths = normalize_create_sources(&options.sources)?;
    validate_create_source_files(&source_paths)?;
    let office_create_script_path =
        resolve_create_script_path(options.office_create_script.unwrap_or_default())?;
    let resolved = ResolvedCreateOptions {
        family,
        output_path: output_path.to_string(),
        source_paths,
        extract_bin_path: options.extract_bin.unwrap_or_default().trim().to_string(),
        office_create_script_path,
        enable_vba_object_model_access: options.enable_vba_object_model_access,
        visible: options.visible,
        force: options.force,
    };
    let script_result = invoke_create_script(&resolved)?;
    complete_create_result(script_result, &resolved)
}

fn normalize_create_family(value: &str, output_path: &str) -> CliResult<(String, bool)> {
    let value = value.trim().to_ascii_lowercase();
    if value.is_empty() {
        return match extension_lower(output_path).as_str() {
            ".xlsm" => Ok(("xlsx".to_string(), true)),
            ".pptm" => Ok(("pptx".to_string(), true)),
            _ => Err(CliError::invalid_args(
                "--family is required when output extension is not .xlsm or .pptm",
            )),
        };
    }
    match value.as_str() {
        "xlsx" | "xlsm" | "excel" | "workbook" => Ok(("xlsx".to_string(), false)),
        "pptx" | "pptm" | "powerpoint" | "presentation" | "deck" => Ok(("pptx".to_string(), false)),
        _ => Err(CliError::invalid_args("--family must be xlsx or pptx")),
    }
}

fn validate_create_output_extension(family: &str, output_path: &str) -> CliResult<()> {
    let extension = extension_lower(output_path);
    match family {
        "xlsx" if extension != ".xlsm" => Err(CliError::invalid_args(
            "output for family xlsx must end with .xlsm",
        )),
        "pptx" if extension != ".pptm" => Err(CliError::invalid_args(
            "output for family pptx must end with .pptm",
        )),
        "xlsx" | "pptx" => Ok(()),
        _ => Err(CliError::invalid_args("--family must be xlsx or pptx")),
    }
}

fn normalize_create_sources(values: &[String]) -> CliResult<Vec<String>> {
    let mut out = Vec::new();
    for value in values {
        out.extend(expand_create_source_value(value));
    }
    if out.is_empty() {
        return Err(CliError::invalid_args(
            "--source is required (repeat it for each .bas/.cls file; use vba create --pure for XLSM .frm UserForms)",
        ));
    }
    Ok(out)
}

fn expand_create_source_value(value: &str) -> Vec<String> {
    let value = value.trim();
    if value.is_empty() {
        return Vec::new();
    }
    if fs::metadata(value).is_ok() {
        return vec![value.to_string()];
    }
    value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn validate_create_source_files(paths: &[String]) -> CliResult<()> {
    for path in paths {
        match fs::metadata(path) {
            Ok(_) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Err(CliError::file_not_found(format!(
                    "VBA source file not found: {path}"
                )));
            }
            Err(err) => {
                return Err(CliError::unexpected(format!(
                    "failed to stat VBA source file {path}: {err}"
                )));
            }
        }
        match extension_lower(path).as_str() {
            ".bas" | ".cls" => {}
            _ => {
                return Err(CliError::invalid_args(format!(
                    "legacy Office-COM VBA create source must be .bas or .cls: {path}; use vba create --pure for XLSM .frm UserForms"
                )));
            }
        }
    }
    Ok(())
}

fn resolve_create_script_path(override_path: &str) -> CliResult<String> {
    let override_path = override_path.trim();
    if !override_path.is_empty() {
        let abs = absolute_path(override_path);
        match fs::metadata(&abs) {
            Ok(info) if info.is_dir() => {
                return Err(CliError::invalid_args(format!(
                    "--office-create-script must be a file: {override_path}"
                )));
            }
            Ok(_) => return Ok(abs.to_string_lossy().to_string()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Err(CliError::file_not_found(format!(
                    "--office-create-script not found: {override_path}"
                )));
            }
            Err(err) => {
                return Err(CliError::unexpected(format!(
                    "failed to stat --office-create-script {override_path}: {err}"
                )));
            }
        }
    }

    let mut candidates = Vec::new();
    if let Ok(cwd) = env::current_dir() {
        candidates.extend(create_script_candidates_from(&cwd));
    }
    if let Ok(exe) = env::current_exe()
        && let Some(parent) = exe.parent()
    {
        candidates.extend(create_script_candidates_from(parent));
    }
    let mut seen = BTreeSet::new();
    for candidate in candidates {
        let abs = absolute_path(&candidate);
        let key = abs.to_string_lossy().to_ascii_lowercase();
        if !seen.insert(key) {
            continue;
        }
        if fs::metadata(&abs).is_ok_and(|info| !info.is_dir()) {
            return Ok(abs.to_string_lossy().to_string());
        }
    }
    Err(CliError::file_not_found(format!(
        "{OFFICE_CREATE_SCRIPT_FILE} not found; run from the ooxml-cli checkout or pass --office-create-script .\\tools\\{OFFICE_CREATE_SCRIPT_FILE}"
    )))
}

fn create_script_candidates_from(start: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut dir = start.to_path_buf();
    loop {
        out.push(
            dir.join(OFFICE_CREATE_SCRIPT_DIR)
                .join(OFFICE_CREATE_SCRIPT_FILE),
        );
        if !dir.pop() {
            break;
        }
    }
    out
}

fn invoke_create_script(options: &ResolvedCreateOptions) -> CliResult<Value> {
    if !cfg!(windows) {
        return Err(CliError::unsupported_type(
            "vba create requires Windows desktop Microsoft Office; on other platforms create or obtain an Office-authored vbaProject.bin and use ooxml vba attach",
        ));
    }
    let powershell = find_program("powershell.exe").ok_or_else(|| {
        CliError::unsupported_type(
            "vba create requires powershell.exe and desktop Microsoft Office on Windows",
        )
    })?;
    let output = Command::new(powershell)
        .args(build_create_script_args(options)?)
        .output()
        .map_err(|err| {
            CliError::unexpected(format!("vba create Office automation failed: {err}"))
        })?;
    if !output.status.success() {
        let mut detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if detail.is_empty() {
            detail = String::from_utf8_lossy(&output.stdout).trim().to_string();
        }
        let status = output
            .status
            .code()
            .map(|code| format!("exit status {code}"))
            .unwrap_or_else(|| output.status.to_string());
        if detail.is_empty() {
            return Err(CliError::unexpected(format!(
                "vba create Office automation failed: {status}"
            )));
        }
        return Err(CliError::unexpected(format!(
            "vba create Office automation failed: {status}: {detail}"
        )));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(stdout.trim()).map_err(|err| {
        CliError::unexpected(format!("vba create helper returned invalid JSON: {err}"))
    })
}

fn build_create_script_args(options: &ResolvedCreateOptions) -> CliResult<Vec<String>> {
    let source_json = serde_json::to_string(&options.source_paths)
        .map_err(|err| CliError::unexpected(format!("failed to encode source paths: {err}")))?;
    let mut args = vec![
        "-NoProfile".to_string(),
        "-ExecutionPolicy".to_string(),
        "Bypass".to_string(),
        "-File".to_string(),
        options.office_create_script_path.clone(),
        "-Family".to_string(),
        options.family.clone(),
        "-OutputPath".to_string(),
        options.output_path.clone(),
        "-SourcePathJson".to_string(),
        source_json,
    ];
    if !options.extract_bin_path.trim().is_empty() {
        args.push("-ExtractBinPath".to_string());
        args.push(options.extract_bin_path.clone());
    }
    if options.enable_vba_object_model_access {
        args.push("-EnableVbaObjectModelAccess".to_string());
    }
    if options.visible {
        args.push("-Visible".to_string());
    }
    if options.force {
        args.push("-Force".to_string());
    }
    Ok(args)
}

fn complete_create_result(
    script_result: Value,
    options: &ResolvedCreateOptions,
) -> CliResult<Value> {
    let family = string_field(&script_result, "family")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| options.family.clone());
    let output = string_field(&script_result, "output")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| options.output_path.clone());
    let vba_project_bin = string_field(&script_result, "vbaProjectBin")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| options.extract_bin_path.clone());
    let sources = decode_string_list(script_result.get("sources"), "sources")?
        .filter(|items| !items.is_empty())
        .unwrap_or_else(|| options.source_paths.clone());
    let imported_modules =
        decode_imported_modules(script_result.get("importedModules"), "importedModules")?;
    let proof_level = string_field(&script_result, "proofLevel")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "microsoft-office-authored".to_string());

    let mut result = Map::new();
    result.insert("family".to_string(), json!(family));
    result.insert("output".to_string(), json!(output));
    if let Some(value) =
        string_field(&script_result, "outputSha256").filter(|value| !value.trim().is_empty())
    {
        result.insert("outputSha256".to_string(), json!(value));
    }
    if !vba_project_bin.trim().is_empty() {
        result.insert("vbaProjectBin".to_string(), json!(vba_project_bin));
    }
    if let Some(value) =
        string_field(&script_result, "vbaProjectBinSha256").filter(|value| !value.trim().is_empty())
    {
        result.insert("vbaProjectBinSha256".to_string(), json!(value));
    }
    result.insert("sources".to_string(), json!(sources));
    if !imported_modules.is_empty() {
        result.insert(
            "importedModules".to_string(),
            Value::Array(imported_modules),
        );
    }
    result.insert("proofLevel".to_string(), json!(proof_level));
    result.insert("backend".to_string(), json!("windows-office-com"));
    result.insert(
        "officeCreateScriptPath".to_string(),
        json!(options.office_create_script_path),
    );
    result.insert(
        "nextCommands".to_string(),
        create_next_commands(&output, &family, &vba_project_bin),
    );
    result.insert(
        "limitations".to_string(),
        json!([
            "Desktop Office authored and saved the package through COM; macros were imported but not executed.",
            "Macro execution, VBE compile proof, signatures/resigning, forms, and password/protection editing are not verified by vba create."
        ]),
    );
    Ok(Value::Object(result))
}

fn create_next_commands(output: &str, family: &str, vba_project_bin: &str) -> Value {
    let mut next = Map::new();
    next.insert(
        "inspect".to_string(),
        json!(format!("ooxml --json vba inspect {}", command_arg(output))),
    );
    next.insert(
        "list".to_string(),
        json!(format!("ooxml --json vba list {}", command_arg(output))),
    );
    next.insert(
        "validate".to_string(),
        json!(format!("ooxml validate --strict {}", command_arg(output))),
    );
    next.insert(
        "officeCheck".to_string(),
        json!(format!(
            "ooxml --json vba office-check {}",
            command_arg(output)
        )),
    );
    if vba_project_bin.trim().is_empty() {
        next.insert(
            "extractBin".to_string(),
            json!(format!(
                "ooxml --json vba extract-bin {} --out vbaProject.bin",
                command_arg(output)
            )),
        );
    } else {
        next.insert(
            "attachSeed".to_string(),
            json!(standalone_attach_template(vba_project_bin, family)),
        );
    }
    next.insert(
        "readback".to_string(),
        json!(package_readback_command(output, family)),
    );
    Value::Object(next)
}

fn standalone_attach_template(bin_path: &str, family: &str) -> String {
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

fn package_readback_command(output: &str, family: &str) -> String {
    match family {
        "pptx" => format!("ooxml --json pptx slides list {}", command_arg(output)),
        "xlsx" => format!("ooxml --json xlsx sheets list {}", command_arg(output)),
        _ => String::new(),
    }
}

fn decode_string_list(value: Option<&Value>, field: &str) -> CliResult<Option<Vec<String>>> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(vec![value.clone()])),
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| {
                item.as_str()
                    .map(ToOwned::to_owned)
                    .ok_or_else(|| CliError::unexpected(format!("{field}: expected string item")))
            })
            .collect::<CliResult<Vec<_>>>()
            .map(Some),
        Some(_) => Err(CliError::unexpected(format!(
            "{field}: expected string list"
        ))),
    }
}

fn decode_imported_modules(value: Option<&Value>, field: &str) -> CliResult<Vec<Value>> {
    match value {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Object(_)) => Ok(vec![value.cloned().expect("value")]),
        Some(Value::Array(items)) => {
            for item in items {
                if !item.is_object() {
                    return Err(CliError::unexpected(format!(
                        "{field}: expected imported module object"
                    )));
                }
            }
            Ok(items.clone())
        }
        Some(_) => Err(CliError::unexpected(format!(
            "{field}: expected imported module object"
        ))),
    }
}

fn string_field(value: &Value, field: &str) -> Option<String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn extension_lower(path: &str) -> String {
    Path::new(path)
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| format!(".{}", value.to_ascii_lowercase()))
        .unwrap_or_default()
}

fn absolute_path(path: impl AsRef<Path>) -> PathBuf {
    let path = path.as_ref();
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    }
}

fn find_program(name: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    #[cfg(windows)]
    let extensions = env::var_os("PATHEXT")
        .map(|value| {
            env::split_paths(&value)
                .map(|path| path.to_string_lossy().to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| {
            vec![
                ".COM".to_string(),
                ".EXE".to_string(),
                ".BAT".to_string(),
                ".CMD".to_string(),
            ]
        });
    for dir in env::split_paths(&path) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
        #[cfg(windows)]
        if Path::new(name).extension().is_none() {
            for extension in &extensions {
                let candidate = dir.join(format!("{name}{extension}"));
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }
    None
}
