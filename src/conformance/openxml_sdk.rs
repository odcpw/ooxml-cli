use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::process::Command;

const SCHEMA_DIAGNOSTIC_CODE: &str = "OOXML_OPENXML_SDK_SCHEMA";
const RUNNER_DIAGNOSTIC_CODE: &str = "OOXML_OPENXML_SDK_RUNNER_FAILED";
const SKIPPED_DIAGNOSTIC_CODE: &str = "OOXML_OPENXML_SDK_SKIPPED";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ValidatorReport {
    valid: bool,
    error_count: usize,
    schema: String,
    errors: Vec<ValidatorFinding>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ValidatorFinding {
    error_type: String,
    description: String,
    part: String,
    node: String,
    #[serde(rename = "XPath")]
    xpath: String,
}

pub(super) fn schema_check(file: &str) -> Value {
    let doctor_check = crate::doctor::openxml_sdk_validator_check();
    if doctor_check.get("status").and_then(Value::as_str) != Some("ok") {
        return skipped_schema_check(&doctor_check);
    }

    let Some(dotnet_path) = doctor_check.get("dotnetPath").and_then(Value::as_str) else {
        return runner_failure(
            "doctor reported the Open XML SDK validator as available without dotnetPath",
        );
    };
    let Some(validator_dll_path) = doctor_check.get("validatorDllPath").and_then(Value::as_str)
    else {
        return runner_failure(
            "doctor reported the Open XML SDK validator as available without validatorDllPath",
        );
    };

    let output = match Command::new(dotnet_path)
        .args([validator_dll_path, "--json", file])
        .output()
    {
        Ok(output) => output,
        Err(err) => {
            return runner_failure(format!(
                "failed to start Open XML SDK validator via {dotnet_path}: {err}"
            ));
        }
    };
    let report = match parse_validator_output(&output.stdout) {
        Ok(report) => report,
        Err(err) => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let detail = if stderr.is_empty() {
                err
            } else {
                format!("{err}; stderr: {stderr}")
            };
            return runner_failure(detail);
        }
    };

    if report.error_count != report.errors.len() {
        return runner_failure(format!(
            "Open XML SDK validator reported ErrorCount={} but returned {} finding(s)",
            report.error_count,
            report.errors.len()
        ));
    }

    let diagnostics = report
        .errors
        .iter()
        .map(|finding| {
            json!({
                "code": SCHEMA_DIAGNOSTIC_CODE,
                "severity": "error",
                "message": finding.description,
                "part": finding.part,
                "xpath": finding.xpath,
                "node": finding.node,
                "errorType": finding.error_type,
            })
        })
        .collect::<Vec<_>>();
    let passed = output.status.success()
        && report.valid
        && report.error_count == 0
        && diagnostics.is_empty();
    if report.valid && report.error_count == 0 && diagnostics.is_empty() && !output.status.success()
    {
        return runner_failure(format!(
            "Open XML SDK validator reported a clean package but exited with {}",
            output.status
        ));
    }

    let mut check = Map::new();
    check.insert("name".to_string(), json!("schema"));
    check.insert(
        "status".to_string(),
        json!(if passed { "passed" } else { "failed" }),
    );
    if !diagnostics.is_empty() {
        check.insert("diagnostics".to_string(), Value::Array(diagnostics));
    }
    check.insert(
        "schemaCheck".to_string(),
        json!({
            "checked": true,
            "validator": "openxml-sdk",
            "schema": report.schema,
            "valid": report.valid,
            "errorCount": report.error_count,
            "dotnetPath": dotnet_path,
            "validatorDllPath": validator_dll_path,
        }),
    );
    Value::Object(check)
}

fn skipped_schema_check(doctor_check: &Value) -> Value {
    let detail = doctor_check
        .get("detail")
        .and_then(Value::as_str)
        .unwrap_or("Open XML SDK validator is unavailable");
    let mut diagnostic = Map::new();
    diagnostic.insert("code".to_string(), json!(SKIPPED_DIAGNOSTIC_CODE));
    diagnostic.insert("severity".to_string(), json!("info"));
    diagnostic.insert("message".to_string(), json!(detail));
    if let Some(remediation) = doctor_check.get("remediation") {
        diagnostic.insert("remediation".to_string(), remediation.clone());
    }
    if let Some(command) = doctor_check.get("remediationCommand") {
        diagnostic.insert("remediationCommand".to_string(), command.clone());
    }
    json!({
        "name": "schema",
        "status": "skipped",
        "diagnostics": [Value::Object(diagnostic)],
        "schemaCheck": {
            "checked": false,
            "validator": "openxml-sdk",
            "doctorCheck": doctor_check,
        },
    })
}

fn runner_failure(message: impl Into<String>) -> Value {
    json!({
        "name": "schema",
        "status": "failed",
        "diagnostics": [{
            "code": RUNNER_DIAGNOSTIC_CODE,
            "severity": "error",
            "message": message.into(),
        }],
        "schemaCheck": {
            "checked": false,
            "validator": "openxml-sdk",
        },
    })
}

fn parse_validator_output(stdout: &[u8]) -> Result<ValidatorReport, String> {
    serde_json::from_slice(stdout)
        .map_err(|err| format!("Open XML SDK validator returned invalid JSON: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_openxml_sdk_finding_fields() {
        let report = parse_validator_output(
            br#"{
                "Valid": false,
                "ErrorCount": 1,
                "Schema": "Office2019",
                "Errors": [{
                    "ErrorType": "Schema",
                    "Description": "invalid child element pivotTableParts",
                    "Part": "/xl/worksheets/sheet1.xml",
                    "Node": "worksheet",
                    "XPath": "/x:worksheet[1]"
                }]
            }"#,
        )
        .expect("validator JSON");

        assert!(!report.valid);
        assert_eq!(report.error_count, 1);
        assert_eq!(report.schema, "Office2019");
        assert_eq!(report.errors[0].part, "/xl/worksheets/sheet1.xml");
        assert_eq!(report.errors[0].xpath, "/x:worksheet[1]");
        assert_eq!(
            report.errors[0].description,
            "invalid child element pivotTableParts"
        );
    }

    #[test]
    fn skipped_check_carries_doctor_remediation() {
        let doctor_check = json!({
            "status": "warn",
            "detail": "no .NET 8 SDK was reported",
            "remediation": "Install the .NET 8 SDK and build the validator.",
            "remediationCommand": "install-and-build-validator",
        });
        let check = skipped_schema_check(&doctor_check);

        assert_eq!(check["status"], "skipped");
        assert_eq!(check["schemaCheck"]["checked"], false);
        assert_eq!(
            check["diagnostics"][0]["remediationCommand"],
            "install-and-build-validator"
        );
    }
}
