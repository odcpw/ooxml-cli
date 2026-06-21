use serde_json::{Map, Value, json};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{
    CliError, CliResult, EXIT_RENDER_FAILED, EXIT_SUCCESS, EXIT_UNSUPPORTED_TYPE,
    EXIT_VALIDATION_FAILED, InspectPackageKind, detect_inspect_package_type, package_type,
    validate, validate_exit_code, zip_entry_names,
};

use super::inspect::inspect_vba_package;
use super::output::{vba_inspect_command, vba_list_command, vba_validate_command};

const WINDOWS_OFFICE_ORACLE_SCRIPT_DIR: &str = "tools";
const WINDOWS_OFFICE_ORACLE_SCRIPT_FILE: &str = "windows-office-oracle.ps1";

pub(crate) fn vba_office_check(file: &str, out_dir: Option<&str>) -> CliResult<(Value, i32)> {
    let entries = zip_entry_names(file)?;
    let package_kind = detect_inspect_package_type(file, &entries);
    let family = match package_kind {
        InspectPackageKind::Pptx => "pptx",
        InspectPackageKind::Xlsx => "xlsx",
        InspectPackageKind::Docx => "docx",
        InspectPackageKind::Unknown => {
            let detected = package_type(file).unwrap_or("unknown");
            return Err(CliError::unsupported_type(format!(
                "vba office-check supports PPTM/DOCM/XLSM packages only (detected: {detected})"
            )));
        }
    };
    let info = inspect_vba_package(file)?;
    let validation_full = validate(file, true)?;
    let validation = office_validation_summary(&validation_full);
    let package_valid = validate_exit_code(&validation_full, true) == EXIT_SUCCESS;

    if !info.project.as_ref().is_some_and(|project| project.exists) {
        let open_check = skipped_open_check(
            "missing_vba_project",
            "package has no vbaProject.bin part; vba office-check only applies to macro packages",
            None,
        );
        return Ok((
            office_check_result(file, family, false, validation, open_check),
            EXIT_UNSUPPORTED_TYPE,
        ));
    }
    if !package_valid {
        let open_check = skipped_open_check(
            "package_validation_failed",
            "package validation failed; fix validation diagnostics before running an Office-compatible open check",
            None,
        );
        return Ok((
            office_check_result(file, family, false, validation, open_check),
            EXIT_VALIDATION_FAILED,
        ));
    }

    let (open_check, check_err) = run_open_check(file, family, out_dir.unwrap_or_default())?;
    let exit_code = if check_err {
        EXIT_RENDER_FAILED
    } else {
        EXIT_SUCCESS
    };
    Ok((
        office_check_result(file, family, package_valid, validation, open_check),
        exit_code,
    ))
}

fn office_check_result(
    file: &str,
    family: &str,
    package_valid: bool,
    validation: Value,
    open_check: Value,
) -> Value {
    let open_check_status = open_check.get("status").and_then(Value::as_str);
    let office_open_verified = open_check
        .get("officeOpenVerified")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let microsoft_office_verified = open_check
        .get("microsoftOfficeVerified")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let status = if package_valid && open_check_status == Some("passed") && office_open_verified {
        "passed"
    } else if open_check_status == Some("skipped") {
        "skipped"
    } else {
        "failed"
    };
    let compatibility = if microsoft_office_verified {
        "microsoft-office-com-open-check"
    } else {
        "local-engine-open-check"
    };
    let limitations = if microsoft_office_verified {
        microsoft_office_check_limitations()
    } else {
        vba_office_check_limitations()
    };
    json!({
        "file": file,
        "family": family,
        "packageValid": package_valid,
        "validation": validation,
        "openCheck": open_check,
        "inspectCommand": vba_inspect_command(file),
        "validateCommand": vba_validate_command(file),
        "vbaListCommand": vba_list_command(file),
        "limitations": limitations,
        "overallStatus": status,
        "overallVerified": package_valid && office_open_verified,
        "compatibility": compatibility,
        "microsoftOfficeVerified": microsoft_office_verified,
        "macroExecutionVerified": false,
        "macroCompilationVerified": false,
    })
}

fn office_validation_summary(validation: &Value) -> Value {
    let mut object = Map::new();
    if let Some(status) = validation.get("status") {
        object.insert("status".to_string(), status.clone());
    }
    if let Some(diagnostics) = validation.get("diagnostics") {
        object.insert("diagnostics".to_string(), diagnostics.clone());
    }
    if let Some(summary) = validation.get("summary") {
        object.insert("summary".to_string(), summary.clone());
    }
    Value::Object(object)
}

fn skipped_open_check(error_code: &str, error: &str, conversion_format: Option<&str>) -> Value {
    let mut object = Map::new();
    object.insert("status".to_string(), json!("skipped"));
    object.insert("checked".to_string(), json!(false));
    if let Some(conversion_format) = conversion_format {
        object.insert("conversionFormat".to_string(), json!(conversion_format));
    }
    object.insert("officeOpenVerified".to_string(), json!(false));
    object.insert("microsoftOfficeVerified".to_string(), json!(false));
    object.insert("macroExecutionVerified".to_string(), json!(false));
    object.insert("errorCode".to_string(), json!(error_code));
    object.insert("error".to_string(), json!(error));
    object.insert(
        "limitations".to_string(),
        json!(vba_office_check_limitations()),
    );
    Value::Object(object)
}

fn run_open_check(file: &str, family: &str, out_dir: &str) -> CliResult<(Value, bool)> {
    if cfg!(windows)
        && let Some(result) = run_windows_office_oracle_check(file, out_dir)?
    {
        return Ok(result);
    }

    let conversion_format = conversion_format(family)?;
    let Some(engine) = find_program(&["soffice", "libreoffice"]) else {
        let open_check = skipped_open_check(
            "missing_engine",
            "required Office-compatible tool not available: soffice",
            Some(conversion_format),
        );
        return Ok((with_default_open_check_limitations(open_check), true));
    };

    let (out_dir, cleanup_temp_out_dir) = prepare_output_dir(out_dir)?;
    let profile_dir = env::temp_dir().join(format!(
        "ooxml-office-profile-{}-{}",
        std::process::id(),
        unique_suffix()
    ));
    fs::create_dir_all(&profile_dir).map_err(|err| {
        CliError::unexpected(format!("failed to create profile directory: {err}"))
    })?;
    let args = vec![
        libre_office_user_installation_arg(&profile_dir),
        "--headless".to_string(),
        "--convert-to".to_string(),
        conversion_format.to_string(),
        "--outdir".to_string(),
        out_dir.to_string_lossy().to_string(),
        file.to_string(),
    ];
    let output = Command::new(&engine).args(&args).output();
    let _ = fs::remove_dir_all(&profile_dir);
    let engine_name = engine
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("soffice")
        .to_string();
    let mut result = Map::new();
    result.insert("status".to_string(), json!("skipped"));
    result.insert("checked".to_string(), json!(false));
    result.insert("engine".to_string(), json!(engine_name));
    result.insert("method".to_string(), json!("libreoffice-headless-convert"));
    result.insert("conversionFormat".to_string(), json!(conversion_format));
    result.insert("officeOpenVerified".to_string(), json!(false));
    result.insert("microsoftOfficeVerified".to_string(), json!(false));
    result.insert("macroExecutionVerified".to_string(), json!(false));
    result.insert(
        "limitations".to_string(),
        json!(officecheck_default_limitations()),
    );

    let outcome = match output {
        Ok(output) if output.status.success() => {
            result.insert("checked".to_string(), json!(true));
            match find_converted_output(&out_dir, file, conversion_format) {
                Ok((path, size)) => {
                    result.insert("status".to_string(), json!("passed"));
                    result.insert("officeOpenVerified".to_string(), json!(true));
                    result.insert("outputBytes".to_string(), json!(size));
                    if !cleanup_temp_out_dir {
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
            let error = if !stderr.is_empty() {
                format!("{} failed: {stderr}", engine.to_string_lossy())
            } else if !stdout.is_empty() {
                format!("{} failed: {stdout}", engine.to_string_lossy())
            } else {
                format!(
                    "{} failed: exit status {}",
                    engine.to_string_lossy(),
                    output.status.code().unwrap_or(-1)
                )
            };
            result.insert("status".to_string(), json!("failed"));
            result.insert("checked".to_string(), json!(true));
            result.insert("errorCode".to_string(), json!("engine_failed"));
            result.insert("error".to_string(), json!(error));
            Ok((Value::Object(result), true))
        }
        Err(err) => {
            result.insert("status".to_string(), json!("failed"));
            result.insert("errorCode".to_string(), json!("engine_failed"));
            result.insert(
                "error".to_string(),
                json!(format!("{} failed: {err}", engine.to_string_lossy())),
            );
            Ok((Value::Object(result), true))
        }
    };
    if cleanup_temp_out_dir {
        let _ = fs::remove_dir_all(&out_dir);
    }
    outcome
}

fn run_windows_office_oracle_check(file: &str, out_dir: &str) -> CliResult<Option<(Value, bool)>> {
    let Some(powershell) = find_program(&["powershell.exe", "powershell"]) else {
        return Ok(None);
    };
    let Some(script_path) = resolve_windows_office_oracle_script() else {
        return Ok(None);
    };
    let repo_root = script_path
        .parent()
        .and_then(Path::parent)
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    let (out_dir, cleanup_temp_out_dir) = prepare_output_dir(out_dir)?;
    let summary_path = out_dir.join("summary.json");
    let output = Command::new(powershell)
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
            &script_path.to_string_lossy(),
            "-InputFile",
            file,
            "-RepoRoot",
            &repo_root.to_string_lossy(),
            "-OutputDir",
            &out_dir.to_string_lossy(),
            "-TimeoutSeconds",
            "120",
        ])
        .output();
    let outcome = match output {
        Ok(output) if summary_path.is_file() => {
            let summary = fs::read_to_string(&summary_path).map_err(|err| {
                CliError::unexpected(format!("failed to read Office oracle summary: {err}"))
            })?;
            let summary = summary.trim_start_matches('\u{feff}');
            let summary: Value = serde_json::from_str(summary).map_err(|err| {
                CliError::unexpected(format!("Office oracle summary was invalid JSON: {err}"))
            })?;
            let entry = if let Some(first) = summary.as_array().and_then(|items| items.first()) {
                first
            } else if summary.is_object() {
                &summary
            } else {
                return Err(CliError::unexpected(
                    "Office oracle summary did not contain a file result",
                ));
            };
            Ok(windows_office_open_check_from_summary(
                entry,
                &summary_path,
                output.status.success(),
            ))
        }
        Ok(output) => {
            let detail = process_output_detail(&output);
            Ok(Some((
                windows_office_open_check_failed(
                    "office_oracle_failed",
                    &format!("Microsoft Office oracle did not write summary.json: {detail}"),
                ),
                true,
            )))
        }
        Err(err) => Ok(Some((
            windows_office_open_check_failed(
                "office_oracle_failed",
                &format!("failed to run Microsoft Office oracle: {err}"),
            ),
            true,
        ))),
    };
    if cleanup_temp_out_dir {
        let _ = fs::remove_dir_all(&out_dir);
    }
    outcome
}

fn windows_office_open_check_from_summary(
    entry: &Value,
    summary_path: &Path,
    process_succeeded: bool,
) -> Option<(Value, bool)> {
    let status = entry
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("failed");
    let passed = status == "passed" && process_succeeded;
    let mut result = Map::new();
    result.insert("status".to_string(), json!(status));
    result.insert("checked".to_string(), json!(true));
    result.insert("engine".to_string(), json!("Microsoft Office"));
    result.insert("method".to_string(), json!("windows-office-com-open"));
    result.insert("officeOpenVerified".to_string(), json!(passed));
    result.insert("microsoftOfficeVerified".to_string(), json!(passed));
    result.insert("macroExecutionVerified".to_string(), json!(false));
    result.insert(
        "summaryPath".to_string(),
        json!(summary_path.to_string_lossy()),
    );
    for field in [
        "officeApplication",
        "officeVersion",
        "officeBuild",
        "elapsedMs",
        "errorType",
        "errorMessage",
    ] {
        if let Some(value) = entry.get(field) {
            result.insert(field.to_string(), value.clone());
        }
    }
    result.insert(
        "limitations".to_string(),
        json!(microsoft_office_check_limitations()),
    );
    Some((Value::Object(result), !passed))
}

fn windows_office_open_check_failed(error_code: &str, error: &str) -> Value {
    json!({
        "status": "failed",
        "checked": true,
        "engine": "Microsoft Office",
        "method": "windows-office-com-open",
        "officeOpenVerified": false,
        "microsoftOfficeVerified": false,
        "macroExecutionVerified": false,
        "errorCode": error_code,
        "error": error,
        "limitations": microsoft_office_check_limitations(),
    })
}

fn resolve_windows_office_oracle_script() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(cwd) = env::current_dir() {
        candidates.extend(windows_office_oracle_script_candidates_from(&cwd));
    }
    if let Ok(exe) = env::current_exe()
        && let Some(parent) = exe.parent()
    {
        candidates.extend(windows_office_oracle_script_candidates_from(parent));
    }
    for candidate in candidates {
        let abs = absolute_path(&candidate);
        if fs::metadata(&abs).is_ok_and(|info| !info.is_dir()) {
            return Some(abs);
        }
    }
    None
}

fn windows_office_oracle_script_candidates_from(start: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut dir = start.to_path_buf();
    loop {
        out.push(
            dir.join(WINDOWS_OFFICE_ORACLE_SCRIPT_DIR)
                .join(WINDOWS_OFFICE_ORACLE_SCRIPT_FILE),
        );
        if !dir.pop() {
            break;
        }
    }
    out
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
    let entries = fs::read_dir(out_dir)
        .map_err(|err| format!("failed to inspect output directory: {err}"))?;
    for entry in entries.flatten() {
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

fn conversion_format(family: &str) -> CliResult<&'static str> {
    match family {
        "pptx" => Ok("pdf"),
        "xlsx" => Ok("csv"),
        "docx" => Ok("pdf"),
        _ => Err(CliError::unexpected(format!(
            "office open-check supports pptx/pptm, docx/docm, and xlsx/xlsm only (family {family:?})"
        ))),
    }
}

fn with_default_open_check_limitations(mut value: Value) -> Value {
    if let Value::Object(map) = &mut value {
        map.insert(
            "limitations".to_string(),
            json!(officecheck_default_limitations()),
        );
    }
    value
}

fn vba_office_check_limitations() -> Vec<&'static str> {
    vec![
        "LibreOffice/soffice load and conversion is compatibility evidence, not Microsoft Office proof.",
        "Macros are not executed, compiled, or security-reviewed by this check.",
    ]
}

fn officecheck_default_limitations() -> Vec<&'static str> {
    vec![
        "LibreOffice/soffice load and conversion is compatibility evidence, not Microsoft Office proof.",
        "Macros are not executed or compiled by this check.",
    ]
}

fn microsoft_office_check_limitations() -> Vec<&'static str> {
    vec![
        "Microsoft Office opened the package through COM; macros were not executed.",
        "This check does not prove VBA compilation, signatures/resigning, form behavior, or macro security safety.",
    ]
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

fn process_output_detail(output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !stderr.is_empty() {
        return stderr;
    }
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !stdout.is_empty() {
        return stdout;
    }
    output
        .status
        .code()
        .map(|code| format!("exit status {code}"))
        .unwrap_or_else(|| output.status.to_string())
}

fn find_program(candidates: &[&str]) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    for dir in env::split_paths(&path) {
        for candidate in candidates {
            let path = dir.join(candidate);
            if path.is_file() {
                return Some(path);
            }
            #[cfg(windows)]
            if Path::new(candidate).extension().is_none() {
                for extension in [".com", ".exe", ".bat", ".cmd"] {
                    let path = dir.join(format!("{candidate}{extension}"));
                    if path.is_file() {
                        return Some(path);
                    }
                }
            }
        }
    }
    None
}

fn libre_office_user_installation_arg(profile_dir: &Path) -> String {
    let path = profile_dir.to_string_lossy().replace('\\', "/");
    if cfg!(windows) && path.as_bytes().get(1) == Some(&b':') {
        format!("-env:UserInstallation=file:///{path}")
    } else {
        format!("-env:UserInstallation=file://{path}")
    }
}

fn unique_suffix() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}
