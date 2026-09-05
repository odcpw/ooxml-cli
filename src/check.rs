mod finding;
#[cfg(test)]
mod remediation;
mod xlsx;

use finding::CheckFinding;
use serde_json::{Map, Value, json};
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use crate::cli_dispatch::{DispatchBody, DispatchOutput};
use crate::{
    CliError, CliResult, EXIT_SUCCESS, EXIT_VALIDATION_FAILED, GlobalFlags, InspectPackageKind,
    command_arg, detect_inspect_package_type, has_flag, parse_string_flag, reject_unknown_flags,
    zip_entry_names,
};

const SCHEMA_VERSION: &str = "ooxml-cli.check.v1";
const PROOF_DOCS: &str = "docs/testing-strategy.md";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OpenXmlSdkMode {
    Auto,
    Require,
    Skip,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FailOn {
    Error,
    Warning,
}

#[derive(Clone, Copy, Debug)]
struct CheckOptions {
    render: bool,
    openxml_sdk: OpenXmlSdkMode,
    fail_on: FailOn,
}

pub(crate) fn dispatch(
    flags: &GlobalFlags,
    file: &str,
    args: &[String],
) -> CliResult<DispatchOutput> {
    let options = parse_options(args)?;
    let report = run(file, options)?;
    let exit_code = report_exit_code(&report, options.fail_on);
    let body = if flags.format_text && !flags.json {
        DispatchBody::Text(text_report(&report))
    } else {
        DispatchBody::Json(report)
    };
    Ok(DispatchOutput { body, exit_code })
}

/// Stable Serve/MCP hook: adapters supply a working package path and JSON flags,
/// while every proof source still emits the same `CheckFinding` contract.
pub(crate) fn inspect(working: &str, args: &Value) -> CliResult<Value> {
    let object = args
        .as_object()
        .ok_or_else(|| CliError::invalid_args("check inspect args must be a JSON object"))?;
    let allowed = ["render", "openxml-sdk", "openxmlSdk", "fail-on", "failOn"];
    if let Some(key) = object.keys().find(|key| !allowed.contains(&key.as_str())) {
        return Err(CliError::invalid_args(format!(
            "unknown check inspect argument: {key}"
        )));
    }
    let render = object
        .get("render")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let sdk = object
        .get("openxml-sdk")
        .or_else(|| object.get("openxmlSdk"))
        .and_then(Value::as_str)
        .unwrap_or("auto");
    let fail_on = object
        .get("fail-on")
        .or_else(|| object.get("failOn"))
        .and_then(Value::as_str)
        .unwrap_or("error");
    run(
        working,
        CheckOptions {
            render,
            openxml_sdk: parse_sdk_mode(sdk)?,
            fail_on: parse_fail_on(fail_on)?,
        },
    )
}

fn parse_options(args: &[String]) -> CliResult<CheckOptions> {
    reject_unknown_flags(args, &["--openxml-sdk", "--fail-on"], &["--render"])?;
    Ok(CheckOptions {
        render: has_flag(args, "--render"),
        openxml_sdk: parse_sdk_mode(
            parse_string_flag(args, "--openxml-sdk")?
                .as_deref()
                .unwrap_or("auto"),
        )?,
        fail_on: parse_fail_on(
            parse_string_flag(args, "--fail-on")?
                .as_deref()
                .unwrap_or("error"),
        )?,
    })
}

fn parse_sdk_mode(value: &str) -> CliResult<OpenXmlSdkMode> {
    match value {
        "auto" => Ok(OpenXmlSdkMode::Auto),
        "require" => Ok(OpenXmlSdkMode::Require),
        "skip" => Ok(OpenXmlSdkMode::Skip),
        _ => Err(CliError::invalid_args(
            "--openxml-sdk must be auto, require, or skip",
        )),
    }
}

fn parse_fail_on(value: &str) -> CliResult<FailOn> {
    match value {
        "error" => Ok(FailOn::Error),
        "warning" => Ok(FailOn::Warning),
        _ => Err(CliError::invalid_args("--fail-on must be error or warning")),
    }
}

fn run(file: &str, options: CheckOptions) -> CliResult<Value> {
    let entries = zip_entry_names(file)?;
    let family = match detect_inspect_package_type(file, &entries) {
        InspectPackageKind::Pptx => "pptx",
        InspectPackageKind::Xlsx => "xlsx",
        InspectPackageKind::Docx => "docx",
        InspectPackageKind::Unknown => {
            return Err(CliError::unsupported_type(
                "check supports PPTX, XLSX, and DOCX packages",
            ));
        }
    };
    let mut findings = Vec::new();
    let mut checks = Map::new();
    checks.insert("structural".to_string(), json!("passed"));

    let strict = crate::validation::validate(file, true)?;
    add_validation_findings(file, &strict, &mut findings);
    let strict_status = if crate::validation::validate_exit_code(&strict, true) == EXIT_SUCCESS {
        "passed"
    } else {
        "failed"
    };
    checks.insert("strict".to_string(), json!(strict_status));

    let schema_requested = match options.openxml_sdk {
        OpenXmlSdkMode::Skip => false,
        OpenXmlSdkMode::Require => true,
        OpenXmlSdkMode::Auto => crate::doctor::openxml_sdk_validator_check()["status"] == "ok",
    };
    let (conformance, schema_status) = run_conformance(file, schema_requested)?;
    checks.insert("conformance".to_string(), conformance["status"].clone());
    add_conformance_findings(
        file,
        &conformance,
        options.openxml_sdk,
        schema_requested,
        &mut findings,
    );
    checks.insert("schema".to_string(), json!(schema_status));

    if family == "pptx" {
        match crate::pptx_validate_layout(file) {
            Ok(layout) => {
                checks.insert(
                    "layout".to_string(),
                    json!(if layout["hasIssues"] == true {
                        "warning"
                    } else {
                        "passed"
                    }),
                );
                add_layout_findings(file, &layout, &mut findings);
            }
            Err(error) => {
                checks.insert("layout".to_string(), json!("failed"));
                findings.push(wrapper_finding(
                    file,
                    "CHECK_LAYOUT_FAILED",
                    error.message,
                    "ooxml --json pptx validate-layout",
                ));
            }
        }
    } else {
        checks.insert("layout".to_string(), json!("not-applicable"));
    }

    match crate::design_check::dispatch(&[file.to_string()]) {
        Ok(design) => {
            checks.insert("design".to_string(), design["status"].clone());
            add_design_findings(&design, &mut findings);
        }
        Err(error) => {
            checks.insert("design".to_string(), json!("failed"));
            findings.push(wrapper_finding(
                file,
                "CHECK_DESIGN_LINT_FAILED",
                error.message,
                "ooxml --json design-check",
            ));
        }
    }

    if family == "xlsx" {
        match xlsx::reference_findings(file, &entries) {
            Ok(reference_findings) => {
                checks.insert(
                    "references".to_string(),
                    json!(if reference_findings
                        .iter()
                        .any(|finding| finding.severity == "error")
                    {
                        "failed"
                    } else {
                        "passed"
                    }),
                );
                findings.extend(reference_findings);
            }
            Err(error) => {
                checks.insert("references".to_string(), json!("failed"));
                findings.push(wrapper_finding(
                    file,
                    "CHECK_XLSX_REFERENCE_SCAN_FAILED",
                    error.message,
                    "ooxml --json xlsx sheets list",
                ));
            }
        }
    } else {
        checks.insert("references".to_string(), json!("not-applicable"));
    }

    let visual_status = if options.render {
        run_render(file, &mut findings)
    } else {
        "skipped".to_string()
    };
    checks.insert("visual".to_string(), json!(visual_status));

    sort_and_dedup(&mut findings);
    let summary = summary(&findings);
    let status = if summary["errors"].as_u64().unwrap_or(0) > 0 {
        "failed"
    } else if summary["warnings"].as_u64().unwrap_or(0) > 0 {
        "warning"
    } else {
        "passed"
    };
    Ok(json!({
        "schemaVersion": SCHEMA_VERSION,
        "file": file,
        "family": family,
        "status": status,
        "failOn": match options.fail_on { FailOn::Error => "error", FailOn::Warning => "warning" },
        "proofLevel": {
            "structural": checks["structural"],
            "strict": checks["strict"],
            "schema": checks["schema"],
            "visual": checks["visual"],
        },
        "checks": checks,
        "summary": summary,
        "findings": findings,
        "checkCommand": format!("ooxml --json check {} --openxml-sdk auto", command_arg(file)),
    }))
}

fn run_conformance(file: &str, schema: bool) -> CliResult<(Value, String)> {
    let mut args = vec!["check".to_string(), file.to_string()];
    if schema {
        args.push("--openxml-sdk".to_string());
    }
    let output = crate::conformance::conformance(
        &GlobalFlags {
            json: true,
            format_text: false,
            format_markdown: false,
            strict: false,
        },
        &args,
    )?;
    let DispatchBody::Json(report) = output.body else {
        return Err(CliError::unexpected(
            "conformance returned text inside check",
        ));
    };
    let schema_status = report["checks"]
        .as_array()
        .and_then(|checks| checks.iter().find(|check| check["name"] == "schema"))
        .and_then(|check| check["status"].as_str())
        .unwrap_or("skipped")
        .to_string();
    Ok((report, schema_status))
}

fn add_validation_findings(file: &str, report: &Value, findings: &mut Vec<CheckFinding>) {
    for diagnostic in array(report, "diagnostics") {
        findings.push(diagnostic_finding(file, diagnostic, false));
    }
}

fn add_conformance_findings(
    file: &str,
    report: &Value,
    sdk_mode: OpenXmlSdkMode,
    schema_requested: bool,
    findings: &mut Vec<CheckFinding>,
) {
    for check in array(report, "checks") {
        let name = check["name"].as_str().unwrap_or_default();
        if matches!(name, "package-open" | "repo-validation") {
            continue;
        }
        for diagnostic in array(check, "diagnostics") {
            let mut finding = diagnostic_finding(file, diagnostic, name == "schema");
            if name == "schema"
                && sdk_mode == OpenXmlSdkMode::Require
                && check["status"] == "skipped"
            {
                finding.severity = "error".to_string();
                finding.code = "CHECK_OPENXML_SDK_REQUIRED".to_string();
                finding.fix_command = diagnostic["remediationCommand"]
                    .as_str()
                    .unwrap_or("ooxml --json doctor --only openxml-sdk-validator")
                    .to_string();
            }
            findings.push(finding);
        }
    }
    if sdk_mode == OpenXmlSdkMode::Auto && !schema_requested {
        let doctor = crate::doctor::openxml_sdk_validator_check();
        findings.push(CheckFinding::new(
            "info",
            "CHECK_OPENXML_SDK_SKIPPED",
            Value::Null,
            Value::Null,
            doctor["detail"]
                .as_str()
                .unwrap_or("Open XML SDK validator is unavailable"),
            doctor["remediationCommand"]
                .as_str()
                .unwrap_or("ooxml --json doctor --only openxml-sdk-validator"),
            PROOF_DOCS,
        ));
    }
}

fn diagnostic_finding(file: &str, diagnostic: &Value, schema: bool) -> CheckFinding {
    let code = diagnostic["code"].as_str().unwrap_or(if schema {
        "CHECK_SCHEMA_FINDING"
    } else {
        "CHECK_VALIDATION_FINDING"
    });
    let mut location = Map::new();
    for key in [
        "xpath",
        "node",
        "element",
        "position",
        "check",
        "styleId",
        "numberingId",
        "errorType",
    ] {
        if let Some(value) = diagnostic.get(key) {
            location.insert(key.to_string(), value.clone());
        }
    }
    let fix_command = diagnostic
        .get("remediationCommand")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| validation_fix_command(file, code));
    CheckFinding::new(
        diagnostic["severity"].as_str().unwrap_or("error"),
        code,
        diagnostic.get("part").cloned().unwrap_or(Value::Null),
        if location.is_empty() {
            Value::Null
        } else {
            Value::Object(location)
        },
        diagnostic["message"]
            .as_str()
            .unwrap_or("proof check reported a finding"),
        fix_command,
        PROOF_DOCS,
    )
}

fn validation_fix_command(file: &str, code: &str) -> String {
    if code.starts_with("DOCX_DANGLING_STYLE") || code.starts_with("DOCX_DANGLING_NUMBERING") {
        format!("ooxml --json docx styles list {}", command_arg(file))
    } else if code.starts_with("XML_") || code.starts_with("OOXML_") {
        format!(
            "ooxml --json repair normalize {} --out {}",
            command_arg(file),
            command_arg(&fixed_path(file, "repaired")),
        )
    } else {
        format!("ooxml --json outline {}", command_arg(file))
    }
}

fn add_layout_findings(file: &str, report: &Value, findings: &mut Vec<CheckFinding>) {
    for slide in array(report, "slideReports") {
        let slide_number = slide["slideNumber"].clone();
        let part = json!(format!(
            "/ppt/slides/slide{}.xml",
            slide_number.as_u64().unwrap_or_default()
        ));
        for (field, code, default_severity) in [
            ("textOverflows", "PPTX_TEXT_OVERFLOW", "error"),
            ("collisions", "PPTX_SHAPE_COLLISION", "warning"),
            ("offSlide", "PPTX_SHAPE_OFF_SLIDE", "error"),
            ("safeMarginViolations", "PPTX_SAFE_MARGIN", "warning"),
        ] {
            for issue in array(slide, field) {
                let severity = match issue["severity"].as_str() {
                    Some("high") => "error",
                    Some("medium" | "low") => "warning",
                    Some(value @ ("error" | "warning" | "info")) => value,
                    _ => default_severity,
                };
                let mut location = Map::new();
                location.insert("slide".to_string(), slide_number.clone());
                for key in [
                    "shapeId",
                    "shapeName",
                    "shapeId1",
                    "shapeId2",
                    "shapeName1",
                    "shapeName2",
                ] {
                    if let Some(value) = issue.get(key) {
                        location.insert(key.to_string(), value.clone());
                    }
                }
                findings.push(CheckFinding::new(
                    severity,
                    code,
                    part.clone(),
                    Value::Object(location),
                    issue["reason"]
                        .as_str()
                        .or_else(|| issue["message"].as_str())
                        .unwrap_or("PowerPoint layout issue"),
                    issue["fixCommand"]
                        .as_str()
                        .unwrap_or_else(|| report["file"].as_str().map_or("", |_| "")),
                    "docs/layout-authoring.md",
                ));
            }
        }
    }
    for finding in findings
        .iter_mut()
        .filter(|finding| finding.code.starts_with("PPTX_") && finding.fix_command.is_empty())
    {
        finding.fix_command = format!("ooxml --json pptx validate-layout {}", command_arg(file));
    }
}

/// Stable design-lint adapter boundary. Design rules may evolve independently,
/// but `check` always projects them into the seven-field `CheckFinding` shape.
fn add_design_findings(report: &Value, findings: &mut Vec<CheckFinding>) {
    for item in array(report, "findings") {
        let code = item["code"].as_str().unwrap_or("DESIGN_CHECK_FINDING");
        let part = item
            .get("part")
            .filter(|part| !part.is_null())
            .cloned()
            .or_else(|| item.get("location")?.get("part").cloned())
            .unwrap_or(Value::Null);
        let mut fix_command = item["fixCommand"].as_str().unwrap_or_default().to_string();
        if code == "DOCX_DANGLING_STYLE"
            && fix_command.contains(" docx styles apply ")
            && !fix_command.contains(" --create-style")
        {
            fix_command.push_str(" --create-style");
        }
        findings.push(CheckFinding::new(
            item["severity"].as_str().unwrap_or("warning"),
            code,
            part,
            item.get("location").cloned().unwrap_or(Value::Null),
            item["message"].as_str().unwrap_or("design check finding"),
            fix_command,
            "docs/bridge-plan-2026-09.md#d8-design-lint",
        ));
    }
}

fn run_render(file: &str, findings: &mut Vec<CheckFinding>) -> String {
    let temp = TempRenderDir::new();
    let args = vec![
        "--out".to_string(),
        temp.path.to_string_lossy().into_owned(),
    ];
    match crate::render::render_command(file, &args) {
        Ok(report) if report["status"] == "ok" => "passed".to_string(),
        Ok(report) if report["status"] == "skipped" => {
            findings.push(CheckFinding::new(
                "warning",
                "CHECK_RENDER_SKIPPED",
                Value::Null,
                Value::Null,
                report["remediation"]
                    .as_str()
                    .unwrap_or("render tools are unavailable"),
                report["doctorCommand"]
                    .as_str()
                    .unwrap_or("ooxml --json doctor --only render-engine,fonts"),
                PROOF_DOCS,
            ));
            "skipped".to_string()
        }
        Ok(report) => {
            findings.push(wrapper_finding(
                file,
                "CHECK_RENDER_FAILED",
                format!("renderer returned status {}", report["status"]),
                "ooxml render",
            ));
            "failed".to_string()
        }
        Err(error) => {
            findings.push(wrapper_finding(
                file,
                "CHECK_RENDER_FAILED",
                error.message,
                "ooxml render",
            ));
            "failed".to_string()
        }
    }
}

fn wrapper_finding(
    file: &str,
    code: &str,
    message: impl Into<String>,
    command: &str,
) -> CheckFinding {
    CheckFinding::new(
        "error",
        code,
        Value::Null,
        Value::Null,
        message,
        format!("{command} {}", command_arg(file)),
        PROOF_DOCS,
    )
}

fn fixed_path(file: &str, suffix: &str) -> String {
    crate::design_check::fixed_output_path(file, suffix)
}

fn sort_and_dedup(findings: &mut Vec<CheckFinding>) {
    findings.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));
    let mut seen = BTreeSet::new();
    findings.retain(|finding| {
        let key = if finding.code == "XLSX_CHART_SOURCE_INVALID" {
            serde_json::to_string(&(
                &finding.severity,
                &finding.code,
                &finding.part,
                &finding.fix_command,
            ))
            .expect("serialize actionable XLSX chart finding key")
        } else if matches!(
            finding.code.as_str(),
            "DOCX_DANGLING_STYLE" | "DOCX_DANGLING_NUMBERING"
        ) {
            serde_json::to_string(&(
                &finding.severity,
                &finding.code,
                &finding.part,
                &finding.message,
            ))
            .expect("serialize semantic DOCX finding key")
        } else {
            finding.dedup_key()
        };
        seen.insert(key)
    });
}

fn summary(findings: &[CheckFinding]) -> Value {
    let errors = findings
        .iter()
        .filter(|finding| finding.severity == "error")
        .count();
    let warnings = findings
        .iter()
        .filter(|finding| finding.severity == "warning")
        .count();
    let info = findings
        .iter()
        .filter(|finding| finding.severity == "info")
        .count();
    json!({
        "errors": errors,
        "warnings": warnings,
        "info": info,
        "total": errors + warnings + info,
    })
}

fn report_exit_code(report: &Value, fail_on: FailOn) -> i32 {
    let errors = report["summary"]["errors"].as_u64().unwrap_or(0);
    let warnings = report["summary"]["warnings"].as_u64().unwrap_or(0);
    if errors > 0 || (fail_on == FailOn::Warning && warnings > 0) {
        EXIT_VALIDATION_FAILED
    } else {
        EXIT_SUCCESS
    }
}

fn text_report(report: &Value) -> String {
    let proof = &report["proofLevel"];
    let summary = &report["summary"];
    let mut output = format!(
        "File: {}\nFamily: {}\nStatus: {}\nProof: structural={}, strict={}, schema={}, visual={}\nSummary: {} error(s), {} warning(s), {} info\n",
        report["file"].as_str().unwrap_or_default(),
        report["family"].as_str().unwrap_or_default(),
        report["status"].as_str().unwrap_or_default(),
        proof["structural"].as_str().unwrap_or_default(),
        proof["strict"].as_str().unwrap_or_default(),
        proof["schema"].as_str().unwrap_or_default(),
        proof["visual"].as_str().unwrap_or_default(),
        summary["errors"],
        summary["warnings"],
        summary["info"],
    );
    for finding in array(report, "findings") {
        output.push_str(&format!(
            "[{}] {}: {}\n  fix: {}\n  docs: {}\n",
            finding["severity"].as_str().unwrap_or_default(),
            finding["code"].as_str().unwrap_or_default(),
            finding["message"].as_str().unwrap_or_default(),
            finding["fixCommand"].as_str().unwrap_or_default(),
            finding["docs"].as_str().unwrap_or_default(),
        ));
    }
    output
}

fn array<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
}

struct TempRenderDir {
    path: PathBuf,
}

impl TempRenderDir {
    fn new() -> Self {
        Self {
            path: std::env::temp_dir().join(format!("ooxml-check-render-{}", std::process::id())),
        }
    }
}

impl Drop for TempRenderDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fail_on_warning_changes_only_exit_decision() {
        let report = json!({"summary": {"errors": 0, "warnings": 1}});
        assert_eq!(report_exit_code(&report, FailOn::Error), EXIT_SUCCESS);
        assert_eq!(
            report_exit_code(&report, FailOn::Warning),
            EXIT_VALIDATION_FAILED
        );
    }

    #[test]
    fn check_finding_serializes_every_stable_field() {
        let value = serde_json::to_value(CheckFinding::new(
            "warning",
            "CODE",
            Value::Null,
            Value::Null,
            "message",
            "ooxml fix",
            "docs/testing-strategy.md",
        ))
        .expect("serialize finding");
        assert_eq!(
            value
                .as_object()
                .expect("finding object")
                .keys()
                .cloned()
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([
                "code".to_string(),
                "docs".to_string(),
                "fixCommand".to_string(),
                "location".to_string(),
                "message".to_string(),
                "part".to_string(),
                "severity".to_string(),
            ])
        );
    }

    #[test]
    fn generated_fix_paths_preserve_forward_slashes() {
        assert_eq!(
            fixed_path(
                "testdata/xlsx/chart-source/missing-sheet.xlsx",
                "chart-source-fixed"
            ),
            "testdata/xlsx/chart-source/missing-sheet.chart-source-fixed.xlsx"
        );
    }
}
