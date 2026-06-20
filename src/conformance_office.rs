use serde_json::{Map, Value, json};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{CliError, CliResult};

const LIMITATIONS: [&str; 2] = [
    "LibreOffice/soffice load and conversion is compatibility evidence, not Microsoft Office proof.",
    "Macros are not executed or compiled by this check.",
];

pub(crate) fn conformance_office_open_check(
    file: &str,
    family: &str,
    out_dir: Option<&str>,
) -> CliResult<Value> {
    if family != "pptx" && family != "xlsx" {
        return Ok(json!({
            "name": "office-open",
            "status": "skipped",
            "diagnostics": [{
                "code": "OOXML_OFFICE_CHECK_UNSUPPORTED",
                "severity": "info",
                "message": format!("office open-check supports pptx/xlsx only, got {family}"),
            }],
        }));
    }

    let conversion_format = conversion_format(family);
    let Some(engine) = find_program(&["soffice", "libreoffice"]) else {
        return Ok(json!({
            "name": "office-open",
            "status": "skipped",
            "diagnostics": [{
                "code": "OOXML_OFFICE_CHECK_SKIPPED",
                "severity": "info",
                "message": "required Office-compatible tool not available: soffice",
            }],
            "officeCheck": skipped_check(conversion_format),
        }));
    };

    let (open_check, failed) = run_libreoffice_check(file, conversion_format, &engine, out_dir)?;
    let status = if failed { "failed" } else { "passed" };
    let mut check = Map::new();
    check.insert("name".to_string(), json!("office-open"));
    check.insert("status".to_string(), json!(status));
    if failed {
        let message = open_check
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("Office-compatible open check failed");
        check.insert(
            "diagnostics".to_string(),
            json!([{
                "code": "OOXML_OFFICE_CHECK_FAILED",
                "severity": "error",
                "message": message,
            }]),
        );
    }
    check.insert("officeCheck".to_string(), open_check);
    Ok(Value::Object(check))
}

fn conversion_format(family: &str) -> &'static str {
    match family {
        "pptx" => "pdf",
        _ => "csv",
    }
}

fn skipped_check(conversion_format: &str) -> Value {
    json!({
        "status": "skipped",
        "checked": false,
        "conversionFormat": conversion_format,
        "officeOpenVerified": false,
        "microsoftOfficeVerified": false,
        "macroExecutionVerified": false,
        "errorCode": "missing_engine",
        "error": "required Office-compatible tool not available: soffice",
        "limitations": LIMITATIONS,
    })
}

fn run_libreoffice_check(
    file: &str,
    conversion_format: &str,
    engine: &Path,
    out_dir: Option<&str>,
) -> CliResult<(Value, bool)> {
    let (out_dir, cleanup_out_dir) = prepare_output_dir(out_dir.unwrap_or_default())?;
    let profile_dir = env::temp_dir().join(format!(
        "ooxml-office-profile-{}-{}",
        std::process::id(),
        unique_suffix()
    ));
    fs::create_dir_all(&profile_dir).map_err(|err| {
        CliError::unexpected(format!("failed to create profile directory: {err}"))
    })?;

    let engine_name = engine
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("soffice");
    let output = Command::new(engine)
        .args([
            libreoffice_user_installation_arg(&profile_dir),
            "--headless".to_string(),
            "--convert-to".to_string(),
            conversion_format.to_string(),
            "--outdir".to_string(),
            out_dir.to_string_lossy().to_string(),
            file.to_string(),
        ])
        .output();
    let _ = fs::remove_dir_all(&profile_dir);

    let mut result = Map::new();
    result.insert("status".to_string(), json!("skipped"));
    result.insert("checked".to_string(), json!(false));
    result.insert("engine".to_string(), json!(engine_name));
    result.insert("method".to_string(), json!("libreoffice-headless-convert"));
    result.insert("conversionFormat".to_string(), json!(conversion_format));
    result.insert("officeOpenVerified".to_string(), json!(false));
    result.insert("microsoftOfficeVerified".to_string(), json!(false));
    result.insert("macroExecutionVerified".to_string(), json!(false));
    result.insert("limitations".to_string(), json!(LIMITATIONS));

    let outcome = match output {
        Ok(output) if output.status.success() => {
            result.insert("checked".to_string(), json!(true));
            match find_converted_output(&out_dir, file, conversion_format) {
                Ok((path, size)) => {
                    result.insert("status".to_string(), json!("passed"));
                    result.insert("officeOpenVerified".to_string(), json!(true));
                    result.insert("outputBytes".to_string(), json!(size));
                    if !cleanup_out_dir {
                        result.insert("outputPath".to_string(), json!(path.to_string_lossy()));
                    }
                    Ok((Value::Object(result), false))
                }
                Err(err) => {
                    result.insert("status".to_string(), json!("failed"));
                    result.insert("checked".to_string(), json!(true));
                    result.insert("errorCode".to_string(), json!("conversion_output_missing"));
                    result.insert("error".to_string(), json!(err));
                    Ok((Value::Object(result), true))
                }
            }
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let detail = if !stderr.is_empty() {
                format!("{engine_name} failed: {stderr}")
            } else if !stdout.is_empty() {
                format!("{engine_name} failed: {stdout}")
            } else {
                format!(
                    "{engine_name} failed: exit status {}",
                    output.status.code().unwrap_or(-1)
                )
            };
            result.insert("status".to_string(), json!("failed"));
            result.insert("checked".to_string(), json!(true));
            result.insert("errorCode".to_string(), json!("engine_failed"));
            result.insert("error".to_string(), json!(detail));
            Ok((Value::Object(result), true))
        }
        Err(err) => {
            result.insert("status".to_string(), json!("failed"));
            result.insert("errorCode".to_string(), json!("engine_failed"));
            result.insert(
                "error".to_string(),
                json!(format!("{engine_name} failed: {err}")),
            );
            Ok((Value::Object(result), true))
        }
    };

    if cleanup_out_dir {
        let _ = fs::remove_dir_all(&out_dir);
    }
    outcome
}

fn prepare_output_dir(out_dir: &str) -> CliResult<(PathBuf, bool)> {
    if !out_dir.trim().is_empty() {
        fs::create_dir_all(out_dir).map_err(|err| {
            CliError::unexpected(format!("failed to create output directory: {err}"))
        })?;
        return Ok((PathBuf::from(out_dir), false));
    }
    let temp = env::temp_dir().join(format!(
        "ooxml-office-check-{}-{}",
        std::process::id(),
        unique_suffix()
    ));
    fs::create_dir_all(&temp).map_err(|err| {
        CliError::unexpected(format!(
            "failed to create temporary output directory: {err}"
        ))
    })?;
    Ok((temp, true))
}

fn find_converted_output(
    out_dir: &Path,
    file: &str,
    format: &str,
) -> Result<(PathBuf, u64), String> {
    let preferred = out_dir.join(format!(
        "{}.{}",
        Path::new(file)
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("output"),
        format
    ));
    if let Ok(info) = fs::metadata(&preferred) {
        if info.len() == 0 {
            return Err(format!(
                "converted output is empty: {}",
                preferred.display()
            ));
        }
        return Ok((preferred, info.len()));
    }
    for entry in fs::read_dir(out_dir)
        .map_err(|err| format!("failed to inspect output directory: {err}"))?
        .flatten()
    {
        let path = entry.path();
        if path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.eq_ignore_ascii_case(format))
            && let Ok(info) = fs::metadata(&path)
            && info.len() > 0
        {
            return Ok((path, info.len()));
        }
    }
    Err(format!(
        "no non-empty {format} output produced in {}",
        out_dir.display()
    ))
}

fn find_program(candidates: &[&str]) -> Option<PathBuf> {
    for candidate in candidates {
        if let Some(path) = find_program_on_path(candidate) {
            return Some(path);
        }
    }
    None
}

fn find_program_on_path(program: &str) -> Option<PathBuf> {
    let candidate = PathBuf::from(program);
    if candidate.components().count() > 1 && is_executable_file(&candidate) {
        return Some(candidate);
    }
    let path_var = env::var_os("PATH")?;
    let extensions = executable_extensions();
    for dir in env::split_paths(&path_var) {
        let direct = dir.join(program);
        if is_executable_file(&direct) {
            return Some(direct);
        }
        if direct.extension().is_none() {
            for extension in &extensions {
                let with_extension = dir.join(format!("{program}{extension}"));
                if is_executable_file(&with_extension) {
                    return Some(with_extension);
                }
            }
        }
    }
    None
}

fn executable_extensions() -> Vec<String> {
    if cfg!(windows) {
        env::var_os("PATHEXT")
            .map(|value| {
                value
                    .to_string_lossy()
                    .split(';')
                    .filter(|item| !item.trim().is_empty())
                    .map(|item| item.to_string())
                    .collect()
            })
            .unwrap_or_else(|| vec![".EXE".to_string(), ".BAT".to_string(), ".CMD".to_string()])
    } else {
        Vec::new()
    }
}

fn is_executable_file(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|info| info.is_file())
}

fn libreoffice_user_installation_arg(profile_dir: &Path) -> String {
    let mut path = profile_dir.to_string_lossy().replace('\\', "/");
    if cfg!(windows) && !path.starts_with('/') {
        path = format!("/{path}");
    }
    format!("-env:UserInstallation=file://{path}")
}

fn unique_suffix() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}
