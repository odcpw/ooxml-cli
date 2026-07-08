use serde_json::{Map, Value, json};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{CliError, CliResult, EXIT_RENDER_FAILED, EXIT_SUCCESS};

const WINDOWS_VBA_RUN_SMOKE_SCRIPT_DIR: &str = "tools";
const WINDOWS_VBA_RUN_SMOKE_SCRIPT_FILE: &str = "windows-office-vba-run-smoke.ps1";

pub(crate) struct VbaRunSmokeOptions<'a> {
    pub(crate) out_dir: Option<&'a str>,
    pub(crate) macro_name: Option<&'a str>,
    pub(crate) expected_cell: Option<&'a str>,
    pub(crate) expected_value: Option<&'a str>,
    pub(crate) smoke_mode: Option<&'a str>,
    pub(crate) timeout_seconds: u32,
    pub(crate) visible: bool,
}

pub(crate) fn vba_run_smoke(
    input_file: Option<&str>,
    options: VbaRunSmokeOptions<'_>,
) -> CliResult<(Value, i32)> {
    if options.timeout_seconds == 0 {
        return Err(CliError::invalid_args(
            "--timeout-seconds must be greater than zero",
        ));
    }
    let smoke_mode = normalize_smoke_mode(options.smoke_mode)?;
    let input_path = if let Some(input_file) = input_file {
        if options.smoke_mode.is_some() {
            return Err(CliError::invalid_args(
                "--smoke-mode is only used when vba run-smoke generates a workbook; omit it when passing an .xlsm file",
            ));
        }
        Some(resolve_input_xlsm_path(input_file)?)
    } else {
        if options
            .macro_name
            .is_some_and(|value| !value.trim().is_empty())
        {
            return Err(CliError::invalid_args(
                "--macro is only supported when passing an existing .xlsm file; generated smoke workbooks always use AgentSmokeRun",
            ));
        }
        None
    };
    if !cfg!(windows) {
        return Err(CliError::unsupported_type(
            "vba run-smoke requires Windows desktop Microsoft Excel because it explicitly executes VBA through Office COM",
        ));
    }
    let Some(powershell) = find_program(&["powershell.exe", "powershell"]) else {
        return Err(CliError::unsupported_type(
            "vba run-smoke requires powershell.exe on Windows",
        ));
    };
    let Some(script_path) = resolve_vba_run_smoke_script() else {
        return Err(CliError::file_not_found(format!(
            "could not find {WINDOWS_VBA_RUN_SMOKE_SCRIPT_FILE}; run from the repo or install the tools directory next to the ooxml binary"
        )));
    };
    let repo_root = script_path
        .parent()
        .and_then(Path::parent)
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    let out_dir = prepare_output_dir(options.out_dir)?;
    let summary_path = out_dir.join("summary.json");
    let mut args = vec![
        "-NoProfile".to_string(),
        "-ExecutionPolicy".to_string(),
        "Bypass".to_string(),
        "-File".to_string(),
        script_path.to_string_lossy().to_string(),
        "-RepoRoot".to_string(),
        repo_root.to_string_lossy().to_string(),
        "-OutputDir".to_string(),
        out_dir.to_string_lossy().to_string(),
        "-SmokeMode".to_string(),
        smoke_mode.to_string(),
        "-TimeoutSeconds".to_string(),
        options.timeout_seconds.to_string(),
    ];
    if let Some(input_file) = input_path {
        args.push("-InputFile".to_string());
        args.push(input_file.to_string_lossy().to_string());
    } else {
        args.push("-BinaryPath".to_string());
        args.push(current_exe_path()?);
    }
    if let Some(macro_name) = options.macro_name.filter(|value| !value.trim().is_empty()) {
        args.push("-MacroName".to_string());
        args.push(macro_name.to_string());
    }
    if let Some(expected_cell) = options
        .expected_cell
        .filter(|value| !value.trim().is_empty())
    {
        args.push("-ExpectedCell".to_string());
        args.push(expected_cell.to_string());
    }
    if let Some(expected_value) = options.expected_value {
        args.push("-ExpectedValue".to_string());
        args.push(expected_value.to_string());
    }
    if options.visible {
        args.push("-Visible".to_string());
    }

    let output = Command::new(&powershell).args(&args).output();
    let report = match output {
        Ok(output) if summary_path.is_file() => {
            vba_run_smoke_report_from_summary(&summary_path, &output, &powershell)?
        }
        Ok(output) => vba_run_smoke_failed_report(
            "smoke_summary_missing",
            format!(
                "VBA run smoke did not write summary.json: {}",
                process_output_detail(&output)
            ),
            &summary_path,
            &powershell,
            Some(output.status.code().unwrap_or(-1)),
        ),
        Err(err) => vba_run_smoke_failed_report(
            "smoke_runner_failed",
            format!("failed to run VBA smoke script: {err}"),
            &summary_path,
            &powershell,
            None,
        ),
    };
    let passed = report
        .get("overallStatus")
        .and_then(Value::as_str)
        .is_some_and(|status| status == "passed");
    Ok((
        report,
        if passed {
            EXIT_SUCCESS
        } else {
            EXIT_RENDER_FAILED
        },
    ))
}

fn resolve_input_xlsm_path(input_file: &str) -> CliResult<PathBuf> {
    let input_path = Path::new(input_file);
    let extension = input_path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    if !extension.eq_ignore_ascii_case("xlsm") {
        return Err(CliError::unsupported_type(
            "vba run-smoke executes Excel macros and requires an .xlsm input when a file is provided",
        ));
    }
    let absolute = absolute_path(input_path);
    if !fs::metadata(&absolute).is_ok_and(|info| info.is_file()) {
        return Err(CliError::file_not_found(format!(
            "vba run-smoke input file was not found: {}",
            absolute.to_string_lossy()
        )));
    }
    Ok(canonical_path_for_powershell(&absolute))
}

fn normalize_smoke_mode(mode: Option<&str>) -> CliResult<&'static str> {
    match mode.unwrap_or("Standard").to_ascii_lowercase().as_str() {
        "standard" => Ok("Standard"),
        "class" => Ok("Class"),
        _ => Err(CliError::invalid_args(
            "--smoke-mode must be Standard or Class",
        )),
    }
}

fn vba_run_smoke_report_from_summary(
    summary_path: &Path,
    output: &Output,
    powershell: &Path,
) -> CliResult<Value> {
    let summary = fs::read_to_string(summary_path)
        .map_err(|err| CliError::unexpected(format!("failed to read smoke summary: {err}")))?;
    let summary = summary.trim_start_matches('\u{feff}');
    let mut summary: Value = serde_json::from_str(summary)
        .map_err(|err| CliError::unexpected(format!("smoke summary was invalid JSON: {err}")))?;
    let status = summary
        .get("result")
        .and_then(|result| result.get("status"))
        .and_then(Value::as_str)
        .unwrap_or("failed");
    let passed = status == "passed" && output.status.success();
    if let Value::Object(object) = &mut summary {
        insert_wrapper_fields(
            object,
            summary_path,
            powershell,
            output.status.code().unwrap_or(-1),
            passed,
        );
        if !output.status.success() {
            object.insert(
                "scriptOutput".to_string(),
                json!(process_output_detail(output)),
            );
        }
    }
    Ok(summary)
}

fn vba_run_smoke_failed_report(
    code: &str,
    message: String,
    summary_path: &Path,
    powershell: &Path,
    exit_code: Option<i32>,
) -> Value {
    let mut object = Map::new();
    object.insert(
        "schemaVersion".to_string(),
        json!("ooxml-cli.vba-run-smoke.v1"),
    );
    object.insert("overallStatus".to_string(), json!("failed"));
    object.insert("overallVerified".to_string(), json!(false));
    object.insert("macroExecutionVerified".to_string(), json!(false));
    object.insert("microsoftOfficeVerified".to_string(), json!(false));
    object.insert(
        "summaryPath".to_string(),
        json!(summary_path.to_string_lossy()),
    );
    object.insert("engine".to_string(), json!("Microsoft Excel"));
    object.insert("method".to_string(), json!("windows-office-com-macro-run"));
    object.insert(
        "powershell".to_string(),
        json!(powershell.to_string_lossy()),
    );
    if let Some(exit_code) = exit_code {
        object.insert("scriptExitCode".to_string(), json!(exit_code));
    }
    object.insert(
        "result".to_string(),
        json!({
            "status": "failed",
            "errorType": code,
            "errorMessage": message,
        }),
    );
    Value::Object(object)
}

fn insert_wrapper_fields(
    object: &mut Map<String, Value>,
    summary_path: &Path,
    powershell: &Path,
    script_exit_code: i32,
    passed: bool,
) {
    object.insert(
        "overallStatus".to_string(),
        json!(if passed { "passed" } else { "failed" }),
    );
    object.insert("overallVerified".to_string(), json!(passed));
    object.insert("macroExecutionVerified".to_string(), json!(passed));
    object.insert("microsoftOfficeVerified".to_string(), json!(passed));
    object.insert("engine".to_string(), json!("Microsoft Excel"));
    object.insert("method".to_string(), json!("windows-office-com-macro-run"));
    object.insert(
        "summaryPath".to_string(),
        json!(summary_path.to_string_lossy()),
    );
    object.insert("scriptExitCode".to_string(), json!(script_exit_code));
    object.insert(
        "powershell".to_string(),
        json!(powershell.to_string_lossy()),
    );
}

fn current_exe_path() -> CliResult<String> {
    env::current_exe()
        .map(|path| path.to_string_lossy().to_string())
        .map_err(|err| CliError::unexpected(format!("failed to locate current executable: {err}")))
}

fn resolve_vba_run_smoke_script() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(cwd) = env::current_dir() {
        candidates.extend(vba_run_smoke_script_candidates_from(&cwd));
    }
    if let Ok(exe) = env::current_exe()
        && let Some(parent) = exe.parent()
    {
        candidates.extend(vba_run_smoke_script_candidates_from(parent));
    }
    for candidate in candidates {
        let abs = absolute_path(&candidate);
        if fs::metadata(&abs).is_ok_and(|info| !info.is_dir()) {
            return Some(abs);
        }
    }
    None
}

fn vba_run_smoke_script_candidates_from(start: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut dir = start.to_path_buf();
    loop {
        out.push(
            dir.join(WINDOWS_VBA_RUN_SMOKE_SCRIPT_DIR)
                .join(WINDOWS_VBA_RUN_SMOKE_SCRIPT_FILE),
        );
        if !dir.pop() {
            break;
        }
    }
    out
}

fn prepare_output_dir(out_dir: Option<&str>) -> CliResult<PathBuf> {
    if let Some(out_dir) = out_dir.filter(|value| !value.trim().is_empty()) {
        let absolute = absolute_path(Path::new(out_dir));
        fs::create_dir_all(&absolute).map_err(|err| {
            CliError::unexpected(format!("failed to create output directory: {err}"))
        })?;
        return Ok(canonical_path_for_powershell(&absolute));
    }
    let temp = env::temp_dir().join(format!(
        "ooxml-vba-run-smoke-{}-{}",
        std::process::id(),
        unique_suffix()
    ));
    fs::create_dir_all(&temp).map_err(|err| {
        CliError::unexpected(format!(
            "failed to create temporary output directory: {err}"
        ))
    })?;
    Ok(temp)
}

fn find_program(candidates: &[&str]) -> Option<PathBuf> {
    for candidate in candidates {
        let path = PathBuf::from(candidate);
        if path.components().count() > 1 && path.is_file() {
            return Some(path);
        }
        if let Some(found) = find_on_path(candidate) {
            return Some(found);
        }
    }
    None
}

fn find_on_path(program: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    for dir in env::split_paths(&path) {
        let candidate = dir.join(program);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn absolute_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir()
            .map(|cwd| cwd.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    }
}

fn canonical_path_for_powershell(path: &Path) -> PathBuf {
    strip_windows_verbatim_prefix(fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf()))
}

fn strip_windows_verbatim_prefix(path: PathBuf) -> PathBuf {
    if !cfg!(windows) {
        return path;
    }
    let value = path.to_string_lossy();
    if let Some(rest) = value.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{rest}"));
    }
    if let Some(rest) = value.strip_prefix(r"\\?\") {
        return PathBuf::from(rest);
    }
    path
}

fn unique_suffix() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

fn process_output_detail(output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !stderr.is_empty() {
        return stderr;
    }
    if !stdout.is_empty() {
        return stdout;
    }
    format!("exit status {}", output.status.code().unwrap_or(-1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static CWD_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn input_xlsm_path_resolves_against_current_directory() {
        let _guard = CWD_LOCK.lock().unwrap();
        let original_cwd = env::current_dir().unwrap();
        let temp = env::temp_dir().join(format!(
            "ooxml-run-smoke-path-test-{}-{}",
            std::process::id(),
            unique_suffix()
        ));
        fs::create_dir_all(&temp).unwrap();
        let workbook = temp.join("book.xlsm");
        fs::write(&workbook, b"dummy").unwrap();
        let expected = canonical_path_for_powershell(&workbook);

        env::set_current_dir(&temp).unwrap();
        let resolved = resolve_input_xlsm_path("book.xlsm").unwrap();
        env::set_current_dir(&original_cwd).unwrap();
        fs::remove_dir_all(&temp).unwrap();

        assert_eq!(resolved, expected);
    }

    #[test]
    fn output_dir_resolves_against_current_directory() {
        let _guard = CWD_LOCK.lock().unwrap();
        let original_cwd = env::current_dir().unwrap();
        let temp = env::temp_dir().join(format!(
            "ooxml-run-smoke-out-test-{}-{}",
            std::process::id(),
            unique_suffix()
        ));
        fs::create_dir_all(&temp).unwrap();

        env::set_current_dir(&temp).unwrap();
        let resolved = prepare_output_dir(Some("proof")).unwrap();
        let expected = canonical_path_for_powershell(&temp.join("proof"));
        env::set_current_dir(&original_cwd).unwrap();
        fs::remove_dir_all(&temp).unwrap();

        assert_eq!(resolved, expected);
    }
}
