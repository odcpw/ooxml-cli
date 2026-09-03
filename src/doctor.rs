use serde_json::{Value, json};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::cli_dispatch::{DispatchBody, DispatchOutput};
use crate::{
    CliError, CliResult, EXIT_SUCCESS, EXIT_UNEXPECTED, GlobalFlags, has_flag, parse_string_flag,
    reject_unknown_flags,
};

const DOCTOR_SCHEMA_VERSION: i64 = 1;
const DOCTOR_VERSION: &str = "1.4.0";
const OPENXML_VALIDATOR_PROJECT: &str = "tools/openxml-validator/openxml-validator.csproj";
const OPENXML_VALIDATOR_DLL: &str =
    "tools/openxml-validator/bin/Release/net8.0/openxml-validator.dll";
const OPENXML_SDK_INSTALL_COMMAND: &str = "curl -fsSL https://dot.net/v1/dotnet-install.sh | bash -s -- --channel 8.0 --install-dir \"$HOME/dotnet\" && \"$HOME/dotnet/dotnet\" build tools/openxml-validator/openxml-validator.csproj --configuration Release";

struct CheckReport {
    id: &'static str,
    title: &'static str,
    status: &'static str,
    severity: &'static str,
    detail: String,
    remediation: Option<String>,
    remediation_command: Option<String>,
    proof_level: Option<&'static str>,
    proof_available: Option<bool>,
    dotnet_path: Option<String>,
    sdk_version: Option<String>,
    sdk_path: Option<String>,
    validator_dll_path: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DotnetSdk {
    version: String,
    path: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DotnetSdkProbe {
    executable: PathBuf,
    sdk: DotnetSdk,
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
    let only = parse_only(args)?;
    validate_only_ids(only.as_ref())?;
    let report = run_report(only);
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
    let only = parse_only(args)?;
    validate_only_ids(only.as_ref())?;
    let value = doctor_health_snapshot(only);
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
                "requiredChecks": ["binary"],
                "command": "ooxml --json conformance check <file>"
            },
            {
                "id": "openxml-sdk-schema",
                "description": "Run package conformance with Microsoft Open XML SDK schema validation.",
                "requiredChecks": ["openxml-sdk-validator"],
                "command": "ooxml --json conformance check <file> --openxml-sdk"
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
            {"name": "--pretty", "type": "bool", "description": "accepted for legacy CLI compatibility"}
        ],
        "notes": [
            "doctor is advisory and read-only; it should not mutate OOXML packages.",
            "Each finding includes remediation or remediationCommand when a deterministic next step is known.",
            "--online is reserved for compatibility and does not perform network access.",
            "Open XML SDK schema proof is available only when the openxml-sdk-validator check status is ok.",
            "conformance check is promoted in Rust for package-open, repo-validation, repair-invariant, optional Open XML SDK schema, and optional local office-open proof.",
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
    if let Some(proof_level) = check.proof_level {
        value["proofLevel"] = json!(proof_level);
    }
    if let Some(proof_available) = check.proof_available {
        value["proofAvailable"] = json!(proof_available);
    }
    if let Some(dotnet_path) = check.dotnet_path {
        value["dotnetPath"] = json!(dotnet_path);
    }
    if let Some(sdk_version) = check.sdk_version {
        value["sdkVersion"] = json!(sdk_version);
    }
    if let Some(sdk_path) = check.sdk_path {
        value["sdkPath"] = json!(sdk_path);
    }
    if let Some(validator_dll_path) = check.validator_dll_path {
        value["validatorDllPath"] = json!(validator_dll_path);
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

pub(crate) fn render_font_warnings() -> Vec<Value> {
    let check = check_fonts();
    if check.status != "warn" {
        return Vec::new();
    }
    vec![json!({
        "code": "OOXML_RENDER_FONTS_UNAVAILABLE",
        "severity": "warning",
        "message": check.detail,
        "remediation": check.remediation,
        "doctorCommand": "ooxml --json doctor --only fonts",
    })]
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
    let project = Path::new(OPENXML_VALIDATOR_PROJECT);
    let validator_dll = Path::new(OPENXML_VALIDATOR_DLL);
    let resolved_validator_dll =
        fs::canonicalize(validator_dll).unwrap_or_else(|_| validator_dll.to_path_buf());
    openxml_sdk_report(
        project,
        project.exists(),
        &resolved_validator_dll,
        validator_dll.is_file(),
        probe_dotnet_8_sdk(),
    )
}

pub(crate) fn openxml_sdk_validator_check() -> Value {
    check_json(check_openxml_sdk_validator())
}

fn openxml_sdk_report(
    project: &Path,
    project_exists: bool,
    validator_dll: &Path,
    validator_dll_exists: bool,
    probe: Option<DotnetSdkProbe>,
) -> CheckReport {
    if !project_exists && !validator_dll_exists {
        return info(
            "openxml-sdk-validator",
            "Open XML SDK validator available",
            format!(
                "{} and {} were not found in this checkout",
                project.display(),
                validator_dll.display()
            ),
        )
        .with_openxml_proof(probe.as_ref(), None);
    }

    let Some(probe) = probe else {
        let detail = if validator_dll_exists {
            format!(
                "validator DLL is {}, but no .NET 8 SDK was reported by dotnet --list-sdks",
                validator_dll.display()
            )
        } else if project_exists {
            "validator project exists, but no .NET 8 SDK was reported by dotnet --list-sdks"
                .to_string()
        } else {
            format!(
                "{} is absent and no .NET 8 SDK was reported by dotnet --list-sdks",
                project.display()
            )
        };
        return warn(
            "openxml-sdk-validator",
            "Open XML SDK validator available",
            detail,
            "Install the .NET 8 SDK and build the Open XML SDK validator.",
            Some(OPENXML_SDK_INSTALL_COMMAND),
        )
        .with_openxml_proof(None, validator_dll_exists.then_some(validator_dll));
    };

    if !validator_dll_exists {
        let build_command = format!(
            "\"{}\" build {} --configuration Release",
            probe.executable.display(),
            project.display()
        );
        let detail = if project_exists {
            format!(
                ".NET SDK {} is available via {}, but {} has not been built",
                probe.sdk.version,
                probe.executable.display(),
                validator_dll.display()
            )
        } else {
            format!(
                ".NET SDK {} is available via {}, but {} and {} are absent",
                probe.sdk.version,
                probe.executable.display(),
                project.display(),
                validator_dll.display()
            )
        };
        return warn(
            "openxml-sdk-validator",
            "Open XML SDK validator available",
            detail,
            "Build the Open XML SDK validator with the detected .NET 8 SDK.",
            Some(&build_command),
        )
        .with_openxml_proof(Some(&probe), None);
    }

    ok(
        "openxml-sdk-validator",
        "Open XML SDK validator available",
        format!(
            ".NET SDK {} is available via {}; validator DLL is {}",
            probe.sdk.version,
            probe.executable.display(),
            validator_dll.display()
        ),
    )
    .with_openxml_proof(Some(&probe), Some(validator_dll))
}

fn probe_dotnet_8_sdk() -> Option<DotnetSdkProbe> {
    for executable in dotnet_candidates() {
        let Ok(output) = Command::new(&executable).arg("--list-sdks").output() else {
            continue;
        };
        if !output.status.success() {
            continue;
        }
        if let Some(sdk) = parse_dotnet_sdks(&output.stdout)
            .into_iter()
            .rev()
            .find(|sdk| sdk.version.split('.').next() == Some("8"))
        {
            return Some(sdk.into_probe(executable));
        }
    }
    None
}

impl DotnetSdk {
    fn into_probe(self, executable: PathBuf) -> DotnetSdkProbe {
        DotnetSdkProbe {
            executable,
            sdk: self,
        }
    }
}

fn parse_dotnet_sdks(stdout: &[u8]) -> Vec<DotnetSdk> {
    String::from_utf8_lossy(stdout)
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let version = line.split_whitespace().next()?;
            let major = version.split('.').next()?.parse::<u32>().ok()?;
            if major == 0 {
                return None;
            }
            let path = line
                .find('[')
                .zip(line.rfind(']'))
                .filter(|(start, end)| start < end)
                .map(|(start, end)| line[start + 1..end].to_string());
            Some(DotnetSdk {
                version: version.to_string(),
                path,
            })
        })
        .collect()
}

fn dotnet_candidates() -> Vec<PathBuf> {
    let binary_name = if cfg!(windows) {
        "dotnet.exe"
    } else {
        "dotnet"
    };
    let mut candidates = Vec::new();
    if let Some(path_dotnet) = find_on_path(binary_name) {
        candidates.push(path_dotnet);
    }
    if let Some(dotnet_root) = env::var_os("DOTNET_ROOT") {
        push_unique_file(
            &mut candidates,
            PathBuf::from(dotnet_root).join(binary_name),
        );
    }
    if let Some(home) = env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }) {
        push_unique_file(
            &mut candidates,
            PathBuf::from(home).join("dotnet").join(binary_name),
        );
    }
    candidates
}

fn push_unique_file(candidates: &mut Vec<PathBuf>, candidate: PathBuf) {
    if candidate.is_file()
        && !candidates
            .iter()
            .any(|existing| canonical_string(existing) == canonical_string(&candidate))
    {
        candidates.push(candidate);
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
        proof_level: None,
        proof_available: None,
        dotnet_path: None,
        sdk_version: None,
        sdk_path: None,
        validator_dll_path: None,
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
        proof_level: None,
        proof_available: None,
        dotnet_path: None,
        sdk_version: None,
        sdk_path: None,
        validator_dll_path: None,
    }
}

fn warn(
    id: &'static str,
    title: &'static str,
    detail: String,
    remediation: &str,
    remediation_command: Option<&str>,
) -> CheckReport {
    CheckReport {
        id,
        title,
        status: "warn",
        severity: "warning",
        detail,
        remediation: Some(remediation.to_string()),
        remediation_command: remediation_command.map(str::to_string),
        proof_level: None,
        proof_available: None,
        dotnet_path: None,
        sdk_version: None,
        sdk_path: None,
        validator_dll_path: None,
    }
}

trait CheckReportExt {
    fn with_remediation_command(self, command: &str) -> Self;
    fn with_openxml_proof(
        self,
        probe: Option<&DotnetSdkProbe>,
        validator_dll: Option<&Path>,
    ) -> Self;
}

impl CheckReportExt for CheckReport {
    fn with_remediation_command(mut self, command: &str) -> Self {
        self.remediation_command = Some(command.to_string());
        self
    }

    fn with_openxml_proof(
        mut self,
        probe: Option<&DotnetSdkProbe>,
        validator_dll: Option<&Path>,
    ) -> Self {
        self.proof_level = Some("schema");
        self.proof_available = Some(self.status == "ok");
        if let Some(probe) = probe {
            self.dotnet_path = Some(probe.executable.display().to_string());
            self.sdk_version = Some(probe.sdk.version.clone());
            self.sdk_path = probe.sdk.path.clone();
        }
        self.validator_dll_path = validator_dll.map(|path| path.display().to_string());
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

fn validate_only_ids(only: Option<&Vec<String>>) -> CliResult<()> {
    let Some(only) = only.filter(|items| !items.is_empty()) else {
        return Ok(());
    };
    let valid = all_checks()
        .into_iter()
        .map(|check| check.id)
        .collect::<Vec<_>>();
    let unknown = only
        .iter()
        .filter(|id| !valid.contains(&id.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if unknown.is_empty() {
        return Ok(());
    }
    Err(CliError::invalid_args(format!(
        "unknown doctor check id(s): {}; valid ids: {}",
        unknown.join(", "),
        valid.join(", ")
    )))
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
        json!({
            "id": "openxml-sdk-validator",
            "title": "Open XML SDK validator available",
            "proofLevel": "schema",
            "availableWhenStatus": "ok"
        }),
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
4. For package proof without desktop Office, run `ooxml validate --strict <file>` and `ooxml --json conformance check <file> --openxml-sdk`.
5. Use Office COM or VBA smoke gates only on Windows hosts where the corresponding checks are ok.

Exit codes:
- 0 means no warn/fail findings.
- 1 means findings are present and the JSON/text report is still on stdout.
- 2 means invalid arguments.

Notes:
The Rust port currently exposes the doctor report, health summary, capabilities, and this guide. It does not mutate files and does not perform network checks.
"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn sdk_probe() -> DotnetSdkProbe {
        DotnetSdkProbe {
            executable: PathBuf::from("/opt/dotnet/dotnet"),
            sdk: DotnetSdk {
                version: "8.0.424".to_string(),
                path: Some("/opt/dotnet/sdk".to_string()),
            },
        }
    }

    #[test]
    fn dotnet_sdk_parser_reads_versions_and_bracketed_paths() {
        let parsed = parse_dotnet_sdks(
            b"6.0.428 [/usr/share/dotnet/sdk]\n8.0.424 [/home/agent/dotnet/sdk]\nmalformed\n",
        );
        assert_eq!(
            parsed,
            vec![
                DotnetSdk {
                    version: "6.0.428".to_string(),
                    path: Some("/usr/share/dotnet/sdk".to_string()),
                },
                DotnetSdk {
                    version: "8.0.424".to_string(),
                    path: Some("/home/agent/dotnet/sdk".to_string()),
                },
            ]
        );
    }

    #[test]
    fn openxml_sdk_report_warns_when_dotnet_has_no_version_8_sdk() {
        let report = check_json(openxml_sdk_report(
            Path::new(OPENXML_VALIDATOR_PROJECT),
            true,
            Path::new(OPENXML_VALIDATOR_DLL),
            true,
            None,
        ));

        assert_eq!(report["status"], "warn");
        assert_eq!(report["proofLevel"], "schema");
        assert_eq!(report["proofAvailable"], false);
        assert!(report.get("sdkVersion").is_none());
        assert_eq!(report["validatorDllPath"], OPENXML_VALIDATOR_DLL);
        let command = report["remediationCommand"]
            .as_str()
            .expect("install remediation command");
        assert!(command.contains("dotnet-install.sh"));
        assert!(command.contains("--channel 8.0"));
        assert!(command.contains("$HOME/dotnet"));
        assert!(command.contains("--configuration Release"));
    }

    #[test]
    fn openxml_sdk_report_warns_with_build_command_when_dll_is_missing() {
        let report = check_json(openxml_sdk_report(
            Path::new(OPENXML_VALIDATOR_PROJECT),
            true,
            Path::new(OPENXML_VALIDATOR_DLL),
            false,
            Some(sdk_probe()),
        ));

        assert_eq!(report["status"], "warn");
        assert_eq!(report["proofAvailable"], false);
        assert_eq!(report["sdkVersion"], "8.0.424");
        assert_eq!(report["dotnetPath"], "/opt/dotnet/dotnet");
        assert_eq!(report["sdkPath"], "/opt/dotnet/sdk");
        let command = report["remediationCommand"]
            .as_str()
            .expect("build remediation command");
        assert!(command.starts_with("\"/opt/dotnet/dotnet\" build"));
        assert!(command.contains(OPENXML_VALIDATOR_PROJECT));
        assert!(command.ends_with("--configuration Release"));
    }

    #[test]
    fn openxml_sdk_report_is_ok_only_with_sdk_and_built_validator() {
        let report = check_json(openxml_sdk_report(
            Path::new(OPENXML_VALIDATOR_PROJECT),
            true,
            Path::new(OPENXML_VALIDATOR_DLL),
            true,
            Some(sdk_probe()),
        ));

        assert_eq!(report["status"], "ok");
        assert_eq!(report["proofLevel"], "schema");
        assert_eq!(report["proofAvailable"], true);
        assert_eq!(report["sdkVersion"], "8.0.424");
        assert_eq!(report["validatorDllPath"], OPENXML_VALIDATOR_DLL);
        assert!(report.get("remediationCommand").is_none());
    }
}
