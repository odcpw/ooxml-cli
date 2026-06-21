use serde_json::{Value, json};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::cli_dispatch::{DispatchBody, DispatchOutput};
use crate::{
    CliResult, EXIT_SUCCESS, EXIT_UNEXPECTED, GlobalFlags, has_flag, parse_string_flag,
    reject_unknown_flags,
};

const DOCTOR_SCHEMA_VERSION: i64 = 1;
const DOCTOR_VERSION: &str = "1.3.0";

struct CheckReport {
    id: &'static str,
    title: &'static str,
    status: &'static str,
    severity: &'static str,
    detail: String,
    remediation: Option<&'static str>,
    remediation_command: Option<&'static str>,
}

pub(crate) fn doctor(flags: &GlobalFlags, args: &[String]) -> CliResult<DispatchOutput> {
    match args {
        [sub, rest @ ..] if sub == "capabilities" => doctor_capabilities(flags, rest),
        [sub, rest @ ..] if sub == "health" => doctor_health(flags, rest),
        [sub, rest @ ..] if sub == "robot-docs" => doctor_robot_docs(rest),
        rest => doctor_report(flags, rest),
    }
}

fn doctor_report(flags: &GlobalFlags, args: &[String]) -> CliResult<DispatchOutput> {
    reject_unknown_flags(
        args,
        &["--only", "--format"],
        &["--json", "--online", "--pretty"],
    )?;
    let report = run_report(parse_only(args)?);
    let exit_code = if report["healthy"].as_bool().unwrap_or(false) {
        EXIT_SUCCESS
    } else {
        EXIT_UNEXPECTED
    };
    if wants_json(flags, args) {
        Ok(DispatchOutput {
            body: DispatchBody::Json(report),
            exit_code,
        })
    } else {
        Ok(DispatchOutput {
            body: DispatchBody::Text(render_report_text(&report)),
            exit_code,
        })
    }
}

fn doctor_health(flags: &GlobalFlags, args: &[String]) -> CliResult<DispatchOutput> {
    reject_unknown_flags(
        args,
        &["--only", "--format"],
        &["--json", "--online", "--pretty"],
    )?;
    let value = doctor_health_snapshot(parse_only(args)?);
    let exit_code = value["exitCode"]
        .as_i64()
        .unwrap_or(i64::from(EXIT_UNEXPECTED)) as i32;
    if wants_json(flags, args) {
        Ok(DispatchOutput {
            body: DispatchBody::Json(value),
            exit_code,
        })
    } else {
        let healthy = value["healthy"].as_bool().unwrap_or(false);
        let findings = value["findings"].as_i64().unwrap_or_default();
        Ok(DispatchOutput {
            body: DispatchBody::Text(format!(
                "healthy={healthy} findings={findings} exitCode={exit_code}\n"
            )),
            exit_code,
        })
    }
}

pub(crate) fn doctor_health_snapshot(only: Option<Vec<String>>) -> Value {
    let report = run_report(only);
    let exit_code = if report["healthy"].as_bool().unwrap_or(false) {
        EXIT_SUCCESS
    } else {
        EXIT_UNEXPECTED
    };
    json!({
        "schemaVersion": report["schemaVersion"],
        "contractVersion": report["schemaVersion"],
        "tool": report["tool"],
        "toolVersion": report["toolVersion"],
        "doctorVersion": report["doctorVersion"],
        "healthy": report["healthy"],
        "summary": report["summary"],
        "findings": report["summary"]["findings"],
        "exitCode": exit_code,
    })
}

fn doctor_capabilities(flags: &GlobalFlags, args: &[String]) -> CliResult<DispatchOutput> {
    reject_unknown_flags(
        args,
        &["--only", "--format"],
        &["--json", "--online", "--pretty"],
    )?;
    let value = json!({
        "tool": "ooxml",
        "doctorVersion": DOCTOR_VERSION,
        "contractVersion": DOCTOR_SCHEMA_VERSION,
        "schemaVersion": DOCTOR_SCHEMA_VERSION,
        "readOnly": true,
        "checks": doctor_check_catalog(),
        "proofLevels": [
            {
                "id": "strict-validation",
                "description": "Run the OOXML package validator in strict mode.",
                "requiredChecks": ["binary"],
                "command": "ooxml validate --strict <file>"
            },
            {
                "id": "repair-conformance",
                "description": "Run the package conformance wrapper.",
                "requiredChecks": ["office-edit-smoke"],
                "command": "ooxml --json conformance check <file>"
            },
            {
                "id": "openxml-sdk-schema",
                "description": "Validate package schema with the Open XML SDK validator helper.",
                "requiredChecks": ["openxml-sdk-validator"],
                "command": "dotnet run --project tools/openxml-validator -- <file>"
            },
            {
                "id": "libreoffice-open-render",
                "description": "Open/render through LibreOffice or soffice when installed.",
                "requiredChecks": ["render-engine"],
                "command": "ooxml --json conformance check --office-check <file>"
            },
            {
                "id": "microsoft-office-com-open",
                "description": "Open a document through Microsoft Office COM automation on Windows.",
                "requiredChecks": ["openxml-sdk-validator", "microsoft-office-com", "office-edit-smoke"],
                "command": "powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\\tools\\windows-office-oracle.ps1 -InputFile <file> -RepoRoot ."
            },
            {
                "id": "microsoft-office-vba-com-open",
                "description": "Open a macro-enabled document through Microsoft Office COM automation on Windows.",
                "requiredChecks": ["openxml-sdk-validator", "microsoft-office-com", "office-vba-smoke"],
                "command": "powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\\tools\\windows-office-oracle.ps1 -InputFile <file> -RepoRoot ."
            }
        ],
        "releaseGates": [
            {
                "id": "check-release-fast",
                "requiresOffice": false,
                "command": "make check-release-fast"
            },
            {
                "id": "check-release-slow",
                "requiresOffice": true,
                "command": "make check-release-slow"
            },
            {
                "id": "check-office-vba-schema",
                "requiresOffice": false,
                "command": "make check-office-vba-schema"
            },
            {
                "id": "check-office-vba-com",
                "requiresOffice": true,
                "command": "make check-office-vba-com"
            }
        ],
        "exitCodes": [
            {"code": 0, "description": "healthy: no findings"},
            {"code": 1, "description": "findings present (advisory; see each finding's remediationCommand)"},
            {"code": 2, "description": "invalid arguments"}
        ],
        "flags": [
            {"name": "--json", "type": "bool", "description": "emit machine-readable JSON"},
            {"name": "--only", "type": "string", "description": "comma-separated check ids to run"},
            {"name": "--online", "type": "bool", "description": "reserved; no network checks are performed"},
            {"name": "--pretty", "type": "bool", "description": "accepted for Go CLI compatibility"}
        ],
        "notes": [
            "doctor is advisory and read-only; it should not mutate OOXML packages.",
            "Each finding includes remediation or remediationCommand when a deterministic next step is known.",
            "--online is reserved for compatibility and does not perform network access.",
            "conformance check is promoted in Rust for package-open, repo-validation, repair-invariant, and optional local office-open proof.",
            "Use check-release-fast before Office-dependent release gates."
        ]
    });
    if wants_json(flags, args) {
        Ok(DispatchOutput {
            body: DispatchBody::Json(value),
            exit_code: EXIT_SUCCESS,
        })
    } else {
        Ok(DispatchOutput {
            body: DispatchBody::Text(render_capabilities_text(&value)),
            exit_code: EXIT_SUCCESS,
        })
    }
}

fn doctor_robot_docs(args: &[String]) -> CliResult<DispatchOutput> {
    reject_unknown_flags(args, &[], &[])?;
    Ok(DispatchOutput {
        body: DispatchBody::Text(DOCTOR_ROBOT_DOCS.to_string()),
        exit_code: EXIT_SUCCESS,
    })
}

fn run_report(only: Option<Vec<String>>) -> Value {
    let only = only.unwrap_or_default();
    let mut checks = all_checks();
    if !only.is_empty() {
        checks.retain(|check| only.iter().any(|id| id == check.id));
    }
    let total = checks.len();
    let ok = checks.iter().filter(|check| check.status == "ok").count();
    let warn = checks.iter().filter(|check| check.status == "warn").count();
    let fail = checks.iter().filter(|check| check.status == "fail").count();
    let info = checks.iter().filter(|check| check.status == "info").count();
    let findings = warn + fail;
    json!({
        "schemaVersion": DOCTOR_SCHEMA_VERSION,
        "tool": "ooxml",
        "toolVersion": env!("CARGO_PKG_VERSION"),
        "doctorVersion": DOCTOR_VERSION,
        "healthy": findings == 0,
        "summary": {
            "total": total,
            "ok": ok,
            "warn": warn,
            "fail": fail,
            "info": info,
            "findings": findings
        },
        "checks": checks.into_iter().map(check_json).collect::<Vec<_>>()
    })
}

fn check_json(check: CheckReport) -> Value {
    let mut value = json!({
        "id": check.id,
        "title": check.title,
        "status": check.status,
        "severity": check.severity,
        "detail": check.detail,
    });
    if let Some(remediation) = check.remediation {
        value["remediation"] = json!(remediation);
    }
    if let Some(command) = check.remediation_command {
        value["remediationCommand"] = json!(command);
    }
    value
}

fn all_checks() -> Vec<CheckReport> {
    vec![
        check_binary(),
        check_render_engine(),
        check_fonts(),
        check_tempdir(),
        check_workdir(),
        check_openxml_sdk_validator(),
        check_microsoft_office_com(),
        check_office_edit_smoke(),
        check_office_vba_smoke(),
    ]
}

fn check_binary() -> CheckReport {
    let current = env::current_exe().ok();
    let path_binary = find_on_path(if cfg!(windows) { "ooxml.exe" } else { "ooxml" });
    match (current, path_binary) {
        (Some(current), Some(path_binary)) => {
            let same = canonical_string(&current) == canonical_string(&path_binary);
            if same {
                ok(
                    "binary",
                    "Installed binary matches this build",
                    format!("PATH resolves to {}", path_binary.display()),
                )
            } else {
                warn(
                    "binary",
                    "Installed binary matches this build",
                    format!(
                        "running {}, but PATH resolves to {}",
                        current.display(),
                        path_binary.display()
                    ),
                    "Rebuild/install the Rust binary or invoke this explicit path.",
                    Some("cargo build --release"),
                )
            }
        }
        (Some(current), None) => warn(
            "binary",
            "Installed binary matches this build",
            format!(
                "running {}, but no ooxml binary was found on PATH",
                current.display()
            ),
            "Add the built ooxml binary to PATH.",
            Some("cargo build --release"),
        ),
        _ => info(
            "binary",
            "Installed binary matches this build",
            "unable to resolve the running executable".to_string(),
        ),
    }
}

fn check_render_engine() -> CheckReport {
    if let Some(path) = find_on_path("soffice").or_else(|| find_on_path("libreoffice")) {
        ok(
            "render-engine",
            "Rendering engine (LibreOffice) available",
            format!("found {}", path.display()),
        )
    } else {
        warn(
            "render-engine",
            "Rendering engine (LibreOffice) available",
            "LibreOffice/soffice was not found on PATH".to_string(),
            "Install LibreOffice and ensure soffice is on PATH.",
            None,
        )
    }
}

fn check_fonts() -> CheckReport {
    if let Some(fc_list) = find_on_path("fc-list") {
        let output = Command::new(fc_list).output();
        match output {
            Ok(output) if output.status.success() && !output.stdout.is_empty() => ok(
                "fonts",
                "Fonts available for rendering",
                "fc-list returned installed fonts".to_string(),
            ),
            Ok(_) => warn(
                "fonts",
                "Fonts available for rendering",
                "fc-list returned no installed fonts".to_string(),
                "Install common document fonts for reliable rendering.",
                None,
            ),
            Err(err) => info(
                "fonts",
                "Fonts available for rendering",
                format!("fc-list could not be executed: {err}"),
            ),
        }
    } else {
        info(
            "fonts",
            "Fonts available for rendering",
            "fc-list was not found; font inventory was skipped".to_string(),
        )
    }
}

fn check_tempdir() -> CheckReport {
    let temp_dir = env::temp_dir();
    match probe_writable(&temp_dir, "ooxml-doctor-temp") {
        Ok(()) => ok(
            "tempdir",
            "Temp directory is writable",
            format!("{} is writable", temp_dir.display()),
        ),
        Err(err) => warn(
            "tempdir",
            "Temp directory is writable",
            format!("{} is not writable: {err}", temp_dir.display()),
            "Set TMP/TEMP to a writable directory.",
            None,
        ),
    }
}

fn check_workdir() -> CheckReport {
    let workdir = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    match probe_writable(&workdir, "ooxml-doctor-workdir") {
        Ok(()) => ok(
            "workdir",
            "Working directory is writable",
            format!("{} is writable", workdir.display()),
        ),
        Err(err) => warn(
            "workdir",
            "Working directory is writable",
            format!("{} is not writable: {err}", workdir.display()),
            "Run from a writable working directory or choose an explicit --out path.",
            None,
        ),
    }
}

fn check_openxml_sdk_validator() -> CheckReport {
    let project = Path::new("tools/openxml-validator/openxml-validator.csproj");
    let dotnet = find_on_path(if cfg!(windows) {
        "dotnet.exe"
    } else {
        "dotnet"
    });
    if project.exists() && dotnet.is_some() {
        ok(
            "openxml-sdk-validator",
            "Open XML SDK validator available",
            "tools/openxml-validator and dotnet are available".to_string(),
        )
    } else if project.exists() {
        warn(
            "openxml-sdk-validator",
            "Open XML SDK validator available",
            "validator project exists, but dotnet was not found".to_string(),
            "Install the .NET SDK.",
            Some("dotnet --list-sdks"),
        )
    } else {
        info(
            "openxml-sdk-validator",
            "Open XML SDK validator available",
            "tools/openxml-validator was not found in this checkout".to_string(),
        )
    }
}

fn check_microsoft_office_com() -> CheckReport {
    if !cfg!(windows) {
        return info(
            "microsoft-office-com",
            "Microsoft Office COM automation available",
            "not running on Windows; Office COM checks are skipped".to_string(),
        );
    }
    let Some(powershell) = powershell_path() else {
        return info(
            "microsoft-office-com",
            "Microsoft Office COM automation available",
            "PowerShell was not found on PATH".to_string(),
        );
    };
    let output = Command::new(powershell)
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "[type]::GetTypeFromProgID('Excel.Application') -ne $null -or [type]::GetTypeFromProgID('PowerPoint.Application') -ne $null -or [type]::GetTypeFromProgID('Word.Application') -ne $null",
        ])
        .output();
    match output {
        Ok(output) if output.status.success() => {
            let text = String::from_utf8_lossy(&output.stdout);
            if text.trim().eq_ignore_ascii_case("true") {
                ok(
                    "microsoft-office-com",
                    "Microsoft Office COM automation available",
                    "at least one Office COM ProgID is registered".to_string(),
                )
            } else {
                info(
                    "microsoft-office-com",
                    "Microsoft Office COM automation available",
                    "Office COM ProgIDs were not found".to_string(),
                )
            }
        }
        Ok(output) => info(
            "microsoft-office-com",
            "Microsoft Office COM automation available",
            format!("PowerShell probe exited with {}", output.status),
        ),
        Err(err) => info(
            "microsoft-office-com",
            "Microsoft Office COM automation available",
            format!("PowerShell probe failed: {err}"),
        ),
    }
}

fn check_office_edit_smoke() -> CheckReport {
    check_script(
        "office-edit-smoke",
        "Windows Office edit smoke gate available",
        Path::new("tools/windows-office-edit-smoke.ps1"),
        "powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\\tools\\windows-office-edit-smoke.ps1 -RepoRoot . -MutationParallelism 4 -RequireOpenXmlSdk -RunConformance -SkipOffice",
    )
}

fn check_office_vba_smoke() -> CheckReport {
    check_script(
        "office-vba-smoke",
        "Windows Office VBA smoke gate available",
        Path::new("tools/windows-office-vba-smoke.ps1"),
        "powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\\tools\\windows-office-vba-smoke.ps1 -RepoRoot . -RequireOpenXmlSdk -SkipOffice -EnableVbaObjectModelAccess",
    )
}

fn check_script(
    id: &'static str,
    title: &'static str,
    script: &Path,
    command: &'static str,
) -> CheckReport {
    if cfg!(windows) && script.exists() && powershell_path().is_some() {
        ok(id, title, format!("{} is available", script.display()))
    } else if script.exists() {
        info(
            id,
            title,
            "script exists, but PowerShell/Windows gate is unavailable".to_string(),
        )
    } else {
        info(
            id,
            title,
            format!("{} was not found in this checkout", script.display()),
        )
        .with_remediation_command(command)
    }
}

fn ok(id: &'static str, title: &'static str, detail: String) -> CheckReport {
    CheckReport {
        id,
        title,
        status: "ok",
        severity: "info",
        detail,
        remediation: None,
        remediation_command: None,
    }
}

fn info(id: &'static str, title: &'static str, detail: String) -> CheckReport {
    CheckReport {
        id,
        title,
        status: "info",
        severity: "info",
        detail,
        remediation: None,
        remediation_command: None,
    }
}

fn warn(
    id: &'static str,
    title: &'static str,
    detail: String,
    remediation: &'static str,
    remediation_command: Option<&'static str>,
) -> CheckReport {
    CheckReport {
        id,
        title,
        status: "warn",
        severity: "warning",
        detail,
        remediation: Some(remediation),
        remediation_command,
    }
}

trait CheckReportExt {
    fn with_remediation_command(self, command: &'static str) -> Self;
}

impl CheckReportExt for CheckReport {
    fn with_remediation_command(mut self, command: &'static str) -> Self {
        self.remediation_command = Some(command);
        self
    }
}

fn parse_only(args: &[String]) -> CliResult<Option<Vec<String>>> {
    Ok(parse_string_flag(args, "--only")?.map(|value| {
        value
            .split(',')
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(ToOwned::to_owned)
            .collect()
    }))
}

fn wants_json(flags: &GlobalFlags, args: &[String]) -> bool {
    flags.json
        || has_flag(args, "--json")
        || args
            .windows(2)
            .any(|pair| (pair[0] == "--format" || pair[0] == "-f") && pair[1] == "json")
        || args
            .iter()
            .any(|arg| arg == "--format=json" || arg == "-f=json")
}

fn find_on_path(name: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    for dir in env::split_paths(&path) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
        if cfg!(windows) && !name.ends_with(".exe") {
            let candidate = dir.join(format!("{name}.exe"));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn powershell_path() -> Option<PathBuf> {
    find_on_path("pwsh").or_else(|| find_on_path("powershell"))
}

fn canonical_string(path: &Path) -> String {
    fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .to_ascii_lowercase()
}

fn probe_writable(dir: &Path, prefix: &str) -> std::io::Result<()> {
    let path = dir.join(format!("{prefix}-{}.tmp", std::process::id()));
    fs::write(&path, b"ooxml doctor")?;
    let _ = fs::remove_file(path);
    Ok(())
}

fn doctor_check_catalog() -> Vec<Value> {
    vec![
        json!({"id": "binary", "title": "Installed binary matches this build"}),
        json!({"id": "render-engine", "title": "Rendering engine (LibreOffice) available"}),
        json!({"id": "fonts", "title": "Fonts available for rendering"}),
        json!({"id": "tempdir", "title": "Temp directory is writable"}),
        json!({"id": "workdir", "title": "Working directory is writable"}),
        json!({"id": "openxml-sdk-validator", "title": "Open XML SDK validator available"}),
        json!({"id": "microsoft-office-com", "title": "Microsoft Office COM automation available"}),
        json!({"id": "office-edit-smoke", "title": "Windows Office edit smoke gate available"}),
        json!({"id": "office-vba-smoke", "title": "Windows Office VBA smoke gate available"}),
    ]
}

fn render_report_text(report: &Value) -> String {
    let summary = &report["summary"];
    let mut out = format!(
        "ooxml doctor: healthy={} total={} ok={} warn={} fail={} info={} findings={}\n",
        report["healthy"].as_bool().unwrap_or(false),
        summary["total"].as_i64().unwrap_or_default(),
        summary["ok"].as_i64().unwrap_or_default(),
        summary["warn"].as_i64().unwrap_or_default(),
        summary["fail"].as_i64().unwrap_or_default(),
        summary["info"].as_i64().unwrap_or_default(),
        summary["findings"].as_i64().unwrap_or_default()
    );
    if let Some(checks) = report["checks"].as_array() {
        for check in checks {
            out.push_str(&format!(
                "- {} [{}]: {}\n",
                check["id"].as_str().unwrap_or_default(),
                check["status"].as_str().unwrap_or_default(),
                check["detail"].as_str().unwrap_or_default()
            ));
            if let Some(command) = check["remediationCommand"].as_str() {
                out.push_str(&format!("  remediationCommand: {command}\n"));
            }
        }
    }
    out
}

fn render_capabilities_text(value: &Value) -> String {
    let check_count = value["checks"].as_array().map(Vec::len).unwrap_or_default();
    let proof_count = value["proofLevels"]
        .as_array()
        .map(Vec::len)
        .unwrap_or_default();
    format!(
        "ooxml doctor capabilities\nschemaVersion: {}\ndoctorVersion: {}\nchecks: {}\nproofLevels: {}\n",
        value["schemaVersion"].as_i64().unwrap_or_default(),
        value["doctorVersion"].as_str().unwrap_or_default(),
        check_count,
        proof_count
    )
}

const DOCTOR_ROBOT_DOCS: &str = r#"OOXML doctor robot guide

Purpose:
Use doctor before release proofs or Office-dependent tasks. The command is read-only and advisory.

Machine-readable commands:
- ooxml --json doctor
- ooxml --json doctor health
- ooxml --json doctor capabilities

Human-readable commands:
- ooxml doctor
- ooxml doctor health
- ooxml doctor robot-docs

Recommended agent flow:
1. Run `ooxml --json doctor health`.
2. If healthy is false, inspect `findings` and then `ooxml --json doctor`.
3. Follow a finding's `remediationCommand` only when it is appropriate for the current task.
4. For package proof without desktop Office, run `ooxml validate --strict <file>` and `ooxml --json conformance check <file>`.
5. Use Office COM or VBA smoke gates only on Windows hosts where the corresponding checks are ok.

Exit codes:
- 0 means no warn/fail findings.
- 1 means findings are present and the JSON/text report is still on stdout.
- 2 means invalid arguments.

Notes:
The Rust port currently exposes the doctor report, health summary, capabilities, and this guide. It does not mutate files and does not perform network checks.
"#;
