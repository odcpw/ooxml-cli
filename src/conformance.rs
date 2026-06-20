use serde_json::{Map, Value, json};

use crate::cli_dispatch::{DispatchBody, DispatchOutput};
use crate::{
    CliError, CliResult, EXIT_SUCCESS, EXIT_VALIDATION_FAILED, GlobalFlags, has_flag, package_type,
    reject_unknown_flags,
};

pub(crate) fn conformance(flags: &GlobalFlags, args: &[String]) -> CliResult<DispatchOutput> {
    match args {
        [sub, rest @ ..] if sub == "coverage" => coverage(flags, rest),
        [sub, rest @ ..] if sub == "check" => check(flags, rest),
        _ => Err(CliError::invalid_args(format!(
            "unsupported Rust-port contract command: conformance {}",
            args.join(" ")
        ))),
    }
}

fn check(flags: &GlobalFlags, args: &[String]) -> CliResult<DispatchOutput> {
    reject_unknown_flags(
        args,
        &["--format", "--office-check-out-dir"],
        &["--json", "--office-check"],
    )?;
    if has_flag(args, "--office-check") {
        return Err(CliError::invalid_args(
            "Rust conformance check has not ported --office-check; command remains unadvertised until office-open parity is available",
        ));
    }
    let file = conformance_check_file_arg(args)?;
    let report = check_report(file)?;
    let exit_code = if report
        .get("status")
        .and_then(Value::as_str)
        .is_some_and(|status| status == "failed")
    {
        EXIT_VALIDATION_FAILED
    } else {
        EXIT_SUCCESS
    };
    if flags.json || has_flag(args, "--json") || local_format_json(args) {
        Ok(DispatchOutput {
            body: DispatchBody::Json(report),
            exit_code,
        })
    } else {
        Ok(DispatchOutput {
            body: DispatchBody::Text(check_text(&report)),
            exit_code,
        })
    }
}

fn conformance_check_file_arg(args: &[String]) -> CliResult<&str> {
    let mut file = None;
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        match arg {
            "--json" | "--office-check" => i += 1,
            "--format" | "-f" | "--office-check-out-dir" => {
                if args.get(i + 1).is_none() {
                    return Err(CliError::invalid_args(format!("{arg} requires a value")));
                }
                i += 2;
            }
            _ if arg.starts_with("--format=") || arg.starts_with("-f=") => i += 1,
            _ if arg.starts_with("--office-check-out-dir=") => i += 1,
            _ if arg.starts_with("--") => {
                return Err(CliError::invalid_args(format!("unknown flag: {arg}")));
            }
            _ => {
                if file.is_some() {
                    return Err(CliError::invalid_args(
                        "conformance check accepts exactly one file argument",
                    ));
                }
                file = Some(arg);
                i += 1;
            }
        }
    }
    file.ok_or_else(|| {
        CliError::invalid_args("conformance check requires exactly one file argument")
    })
}

fn check_report(file: &str) -> CliResult<Value> {
    let mut checks = Vec::new();
    checks.push(json!({"name": "package-open", "status": "passed"}));

    let validation_report = crate::validation::validate(file, false)?;
    let validation_diagnostics = diagnostics_from_report(&validation_report);
    checks.push(check_with_diagnostics(
        "repo-validation",
        validation_diagnostics,
    ));

    let invariant_diagnostics = crate::conformance_invariants::check_repair_invariants(file)?;
    checks.push(check_with_diagnostics(
        "repair-invariants",
        invariant_diagnostics,
    ));

    let family = package_type(file)?;
    Ok(finish_report(file, family, checks))
}

fn diagnostics_from_report(report: &Value) -> Vec<Value> {
    report
        .get("diagnostics")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn check_with_diagnostics(name: &str, diagnostics: Vec<Value>) -> Value {
    let mut check = Map::new();
    check.insert("name".to_string(), json!(name));
    check.insert("status".to_string(), json!(diagnostic_status(&diagnostics)));
    if !diagnostics.is_empty() {
        check.insert("diagnostics".to_string(), Value::Array(diagnostics));
    }
    Value::Object(check)
}

fn diagnostic_status(diagnostics: &[Value]) -> &'static str {
    if diagnostics
        .iter()
        .any(|diag| diag.get("severity").and_then(Value::as_str) == Some("error"))
    {
        "failed"
    } else if diagnostics
        .iter()
        .any(|diag| diag.get("severity").and_then(Value::as_str) == Some("warning"))
    {
        "warning"
    } else {
        "passed"
    }
}

fn finish_report(file: &str, family: &str, checks: Vec<Value>) -> Value {
    let mut passed = 0usize;
    let mut failed = 0usize;
    let mut warnings = 0usize;
    let mut skipped = 0usize;
    let mut errors = 0usize;
    for check in &checks {
        match check
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or_default()
        {
            "failed" => failed += 1,
            "warning" => warnings += 1,
            "skipped" => skipped += 1,
            _ => passed += 1,
        }
        if let Some(diagnostics) = check.get("diagnostics").and_then(Value::as_array) {
            errors += diagnostics
                .iter()
                .filter(|diag| diag.get("severity").and_then(Value::as_str) == Some("error"))
                .count();
        }
    }
    let status = if failed > 0 {
        "failed"
    } else if warnings > 0 {
        "warning"
    } else {
        "passed"
    };
    json!({
        "schemaVersion": "ooxml-cli.conformance.v1",
        "file": file,
        "family": family,
        "status": status,
        "checks": checks,
        "summary": {
            "passed": passed,
            "failed": failed,
            "warnings": warnings,
            "skipped": skipped,
            "errors": errors,
        },
    })
}

fn check_text(report: &Value) -> String {
    let file = report
        .get("file")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let family = report
        .get("family")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let status = report
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let mut out = format!("File: {file}\nFamily: {family}\nStatus: {status}\n\nChecks:\n");
    for check in report
        .get("checks")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let name = check
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let status = check
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or_default();
        out.push_str(&format!("  [{status}] {name}\n"));
        if let Some(diagnostics) = check.get("diagnostics").and_then(Value::as_array) {
            for diagnostic in diagnostics {
                let severity = diagnostic
                    .get("severity")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let code = diagnostic
                    .get("code")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let message = diagnostic
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                out.push_str(&format!("    [{severity}] {code}: {message}\n"));
            }
        }
    }
    out
}

fn coverage(flags: &GlobalFlags, args: &[String]) -> CliResult<DispatchOutput> {
    reject_unknown_flags(args, &["--format"], &["--json"])?;
    let value = coverage_report();
    if flags.json || has_flag(args, "--json") || local_format_json(args) {
        Ok(DispatchOutput {
            body: DispatchBody::Json(value),
            exit_code: EXIT_SUCCESS,
        })
    } else {
        Ok(DispatchOutput {
            body: DispatchBody::Text(coverage_text(&value)),
            exit_code: EXIT_SUCCESS,
        })
    }
}

fn local_format_json(args: &[String]) -> bool {
    args.windows(2)
        .any(|pair| (pair[0] == "--format" || pair[0] == "-f") && pair[1] == "json")
        || args
            .iter()
            .any(|arg| arg == "--format=json" || arg == "-f=json")
}

fn coverage_report() -> Value {
    json!({
        "schemaVersion": "ooxml-cli.conformance.coverage.v1",
        "scope": "pptx-xlsx-office-repair-plus-docx-targeted-invariants",
        "status": "active",
        "harnessStages": [
            {"name": "package-open", "status": "implemented", "evidence": ["pkg/conformance.CheckPackage", "pkg/conformance.TestConformanceGoldenSummary"]},
            {"name": "repo-validation", "status": "implemented", "evidence": ["pkg/validate.ValidatePackage", "pkg/conformance.TestConformanceGoldenSummary"]},
            {"name": "repair-invariants", "status": "implemented", "evidence": ["pkg/conformance.CheckRepairInvariants", "pkg/conformance.TestRepairInvariants*"]},
            {"name": "golden-summary", "status": "implemented", "evidence": ["testdata/golden/repair-conformance-summary.json", "testdata/golden/repair-conformance-office-open-summary.json", "testdata/golden/generated-repair-proof-bundle.json"]},
            {"name": "office-open", "status": "local-engine-optional", "evidence": ["pkg/officecheck", "pkg/conformance.TestConformanceOfficeOpenGoldenSummary", "pkg/conformance.TestCheckPackageOfficeCheckFailureFailsReport", "internal/cli.TestConformanceCheckOfficeCheckJSON", "internal/cli.TestGeneratedRepairConformanceOfficeOpenIfAvailable"]}
        ],
        "repairClasses": repair_classes(),
        "fixtureSets": [
            {
                "name": "committed-pptx-xlsx-fixture-manifest",
                "status": "implemented",
                "families": ["pptx", "xlsx"],
                "evidence": ["TestConformanceCommittedPPTXXLSXFixtureManifest", "testdata/pptx/*", "testdata/xlsx/*"]
            },
            {
                "name": "repair-conformance-golden-summary",
                "status": "implemented",
                "families": ["pptx", "xlsx"],
                "evidence": ["TestConformanceGoldenSummary", "testdata/golden/repair-conformance-summary.json"]
            },
            {
                "name": "office-open-golden-summary",
                "status": "implemented",
                "families": ["pptx", "xlsx"],
                "evidence": ["TestConformanceOfficeOpenGoldenSummary", "testdata/golden/repair-conformance-office-open-summary.json"]
            },
            {
                "name": "generated-output-repair-conformance",
                "status": "implemented",
                "families": ["pptx", "xlsx"],
                "evidence": ["internal/cli.TestGeneratedRepairConformanceGolden", "internal/cli.TestGeneratedRepairConformanceOfficeOpenIfAvailable", "testdata/golden/generated-repair-conformance-summary.json", "testdata/golden/generated-repair-proof-bundle.json"]
            },
            {
                "name": "windows-office-edit-smoke-release-gate",
                "status": "implemented",
                "families": ["docx", "pptx", "xlsx"],
                "evidence": ["Makefile", "tools/windows-office-edit-smoke.ps1", "docs/windows-office-oracle.md"]
            }
        ],
        "knownLimitations": [
            "This is a repair-focused harness for practical generated DOCX/PPTX/XLSX files, not an exhaustive ISO/IEC 29500 conformance suite.",
            "Static repair and generated-output golden summaries remain PPTX/XLSX-only; DOCX coverage is targeted invariants plus Windows edit-smoke release gates.",
            "LibreOffice/soffice open checks are useful local evidence but are not proof that Microsoft Office will avoid a repair prompt.",
            "Real Microsoft Office repair prompts on Windows/macOS remain the final external oracle for user-facing compatibility.",
            "Macro code is not executed or compiled by this harness."
        ]
    })
}

fn repair_classes() -> Vec<Value> {
    vec![
        repair_class(
            "content-types",
            &["pptx", "xlsx"],
            &[
                "OOXML_CONTENT_TYPES_READ_ERROR",
                "OOXML_CONTENT_TYPES_PARSE_ERROR",
                "OOXML_CONTENT_TYPES_ROOT",
                "OOXML_CONTENT_TYPES_DEFAULT_REQUIRED",
                "OOXML_CONTENT_TYPES_DEFAULT_DUPLICATE",
                "OOXML_CONTENT_TYPES_OVERRIDE_REQUIRED",
                "OOXML_CONTENT_TYPES_OVERRIDE_DUPLICATE",
                "OOXML_CONTENT_TYPES_OVERRIDE_TARGET_MISSING",
                "OOXML_CONTENT_TYPES_PART_UNMAPPED",
                "OOXML_CONTENT_TYPE_MISMATCH",
            ],
            &[
                "TestRepairInvariantsCatchContentTypesRootProblem",
                "TestRepairInvariantsCatchContentTypesReadAndParseProblems",
                "TestRepairInvariantsCatchContentTypesPartProblems",
                "TestRepairInvariantsCatchContentTypesCoverageProblems",
                "TestRepairInvariantsCatchKnownContentTypeMismatch",
            ],
        ),
        repair_class(
            "relationships",
            &["pptx", "xlsx"],
            &[
                "OOXML_RELS_READ_ERROR",
                "OOXML_RELS_PARSE_ERROR",
                "OOXML_RELS_ORPHANED",
                "OOXML_RELATIONSHIP_MISSING_ID",
                "OOXML_RELATIONSHIP_DUPLICATE_ID",
                "OOXML_RELATIONSHIP_MISSING_TYPE",
                "OOXML_RELATIONSHIP_MISSING_TARGET",
                "OOXML_RELATIONSHIP_TARGET_MODE",
                "OOXML_RELATIONSHIP_EXTERNAL_MODE_MISSING",
                "OOXML_RELATIONSHIP_TARGET_MISSING",
                "OOXML_RELATIONSHIP_TARGET_CONTENT_TYPE",
            ],
            &[
                "TestRepairInvariantsCatchMalformedRelationshipPart",
                "TestRepairInvariantsCatchRelationshipPartReadError",
                "TestRepairInvariantsCatchRelationshipClosureProblems",
                "TestRepairInvariantsCatchRelationshipTargetContentTypeMismatch",
                "TestConformanceCommittedPPTXXLSXFixtureManifest",
            ],
        ),
        repair_class(
            "part-roots",
            &["pptx", "xlsx"],
            &[
                "XLSX_WORKBOOK_ROOT",
                "XLSX_WORKSHEET_ROOT",
                "XLSX_SHARED_STRINGS_ROOT",
                "XLSX_STYLES_ROOT",
                "XLSX_TABLE_ROOT",
                "XLSX_PIVOT_TABLE_ROOT",
                "XLSX_PIVOT_CACHE_ROOT",
                "XLSX_PIVOT_RECORDS_ROOT",
                "XLSX_CALC_CHAIN_ROOT",
                "PPTX_PRESENTATION_ROOT",
                "PPTX_SLIDE_ROOT",
                "PPTX_SLIDE_LAYOUT_ROOT",
                "PPTX_SLIDE_MASTER_ROOT",
                "XLSX_DRAWING_ROOT",
                "OOXML_CHART_ROOT",
                "OOXML_XML_PARSE_ERROR",
            ],
            &[
                "TestRepairInvariantsCatchKnownPartRootMismatches",
                "TestRepairInvariantsCatchKnownPartRootNamespaceMismatches",
                "TestRepairInvariantsCatchHighValuePartRootMismatches",
                "TestRepairInvariantsCatchSlideLayoutAndMasterProblems",
                "TestRepairInvariantsCatchXMLParseErrors",
            ],
        ),
        repair_class(
            "reference-lists",
            &["pptx", "xlsx"],
            &[
                "XLSX_WORKBOOK_SHEET_REFERENCE",
                "PPTX_PRESENTATION_REFERENCE",
                "XLSX_WORKSHEET_RELATIONSHIP_REFERENCE",
                "XLSX_WORKSHEET_HYPERLINK_REFERENCE",
                "XLSX_WORKSHEET_PIVOT_REFERENCE",
                "XLSX_WORKSHEET_TABLEPARTS_COUNT",
                "PPTX_SLIDE_LAYOUT_MASTER_REFERENCE",
                "PPTX_SLIDE_MASTER_LAYOUT_REFERENCE",
                "OOXML_CHART_RELATIONSHIP_REFERENCE",
            ],
            &[
                "TestRepairInvariantsCatchWorkbookSheetReferenceProblems",
                "TestRepairInvariantsCatchWorksheetRelationshipReferenceProblems",
                "TestRepairInvariantsCatchWorksheetHyperlinkReferenceProblems",
                "TestRepairInvariantsAllowWorksheetHyperlinkReferences",
                "TestRepairInvariantsCatchWorksheetTablePartsCountMismatch",
                "TestRepairInvariantsCatchChartRelationshipReferenceProblems",
                "TestRepairInvariantsCatchPresentationReferenceProblems",
                "TestRepairInvariantsCatchSlideLayoutAndMasterProblems",
                "TestRepairInvariantsAllowSlideLayoutAndMasterReferences",
            ],
        ),
        repair_class(
            "pptx-animations",
            &["pptx"],
            &["PPTX_ANIMATION_TARGET_REFERENCE"],
            &[
                "TestRepairInvariantsCatchSlideAnimationTargetProblems",
                "TestRepairInvariantsAllowSlideAnimationTargets",
            ],
        ),
        repair_class(
            "drawing-media-references",
            &["docx", "pptx", "xlsx"],
            &[
                "OOXML_IMAGE_RELATIONSHIP_REFERENCE",
                "OOXML_IMAGE_PAYLOAD",
                "PPTX_MEDIA_RELATIONSHIP_REFERENCE",
            ],
            &[
                "TestRepairInvariantsCatchDOCXImagePayloadProblems",
                "TestRepairInvariantsAllowDOCXImagePayloadReferences",
                "TestRepairInvariantsCatchDrawingMediaRelationshipProblems",
                "TestRepairInvariantsAllowDrawingMediaRelationshipReferences",
            ],
        ),
        repair_class(
            "chart-external-data",
            &["pptx", "xlsx"],
            &[
                "OOXML_CHART_EXTERNAL_DATA_REFERENCE",
                "OOXML_CHART_EXTERNAL_DATA_EMBEDDED_WORKBOOK_OPEN",
            ],
            &[
                "TestRepairInvariantsCatchChartExternalDataRelationshipProblems",
                "TestRepairInvariantsCatchChartExternalDataCorruptEmbeddedWorkbookBytes",
                "TestRepairInvariantsAllowChartExternalDataRelationshipReferences",
            ],
        ),
        repair_class(
            "chart-axis-references",
            &["pptx", "xlsx"],
            &["OOXML_CHART_AXIS_REFERENCE"],
            &[
                "TestRepairInvariantsCatchChartAxisReferenceProblems",
                "TestRepairInvariantsAllowChartAxisReferences",
            ],
        ),
        repair_class(
            "chart-series-caches",
            &["pptx", "xlsx"],
            &["OOXML_CHART_SERIES_CACHE"],
            &[
                "TestRepairInvariantsCatchChartSeriesCacheProblems",
                "TestRepairInvariantsAllowChartSeriesCaches",
            ],
        ),
        repair_class(
            "xlsx-defined-names",
            &["xlsx"],
            &[
                "XLSX_DEFINED_NAME_REQUIRED",
                "XLSX_DEFINED_NAME_SCOPE",
                "XLSX_DEFINED_NAME_DUPLICATE",
                "XLSX_DEFINED_NAME_REFERENCE",
            ],
            &[
                "TestRepairInvariantsCatchWorkbookDefinedNameProblems",
                "TestRepairInvariantsAllowWorkbookDefinedNames",
            ],
        ),
        repair_class(
            "xlsx-tables",
            &["xlsx"],
            &["XLSX_TABLE_DEFINITION"],
            &[
                "TestRepairInvariantsCatchTableDefinitionProblems",
                "TestRepairInvariantsAllowTableDefinitions",
            ],
        ),
        repair_class(
            "xlsx-pivots",
            &["xlsx"],
            &[
                "XLSX_WORKBOOK_PIVOT_CACHE_REFERENCE",
                "XLSX_PIVOT_TABLE_DEFINITION",
                "XLSX_PIVOT_CACHE_DEFINITION",
                "XLSX_PIVOT_CACHE_RECORDS_REFERENCE",
                "XLSX_PIVOT_RECORDS_DEFINITION",
            ],
            &[
                "TestRepairInvariantsCatchPivotDefinitionProblems",
                "TestRepairInvariantsAllowPivotDefinitions",
            ],
        ),
        repair_class(
            "schema-order",
            &["pptx", "xlsx"],
            &[
                "XLSX_WORKSHEET_CHILD_ORDER",
                "XLSX_TABLE_CHILD_ORDER",
                "XLSX_PIVOT_TABLE_CHILD_ORDER",
                "XLSX_PIVOT_CACHE_CHILD_ORDER",
                "PPTX_SLIDE_CHILD_ORDER",
                "PPTX_SLIDE_LAYOUT_CHILD_ORDER",
                "PPTX_SLIDE_MASTER_CHILD_ORDER",
                "XLSX_DRAWING_ANCHOR_ORDER",
                "XLSX_DRAWING_ANCHOR_REQUIRED",
                "OOXML_CHARTSPACE_CHILD_ORDER",
                "OOXML_CHART_CHILD_ORDER",
                "OOXML_PLOTAREA_CHILD_ORDER",
                "OOXML_BARCHART_CHILD_ORDER",
                "OOXML_LINECHART_CHILD_ORDER",
                "OOXML_AREACHART_CHILD_ORDER",
                "OOXML_PIECHART_CHILD_ORDER",
                "OOXML_SCATTERCHART_CHILD_ORDER",
            ],
            &[
                "TestRepairInvariantsCatchWorksheetChildOrder",
                "TestRepairInvariantsCatchTableDefinitionProblems",
                "TestRepairInvariantsAllowTableDefinitions",
                "TestRepairInvariantsCatchPivotDefinitionProblems",
                "TestRepairInvariantsAllowPivotDefinitions",
                "TestRepairInvariantsCatchSlideChildOrder",
                "TestRepairInvariantsCatchSlideLayoutAndMasterProblems",
                "TestRepairInvariantsAllowSlideLayoutAndMasterReferences",
                "TestRepairInvariantsCatchDrawingAnchorOrderAndRequiredShape",
                "TestRepairInvariantsCatchChartPartOrder",
                "TestRepairInvariantsCatchNestedChartPartOrder",
            ],
        ),
        repair_class(
            "xlsx-counts-styles",
            &["xlsx"],
            &[
                "XLSX_SHARED_STRINGS_COUNTS",
                "XLSX_STYLES_COUNT_MISMATCH",
                "XLSX_CELL_STYLE_REFERENCE",
                "XLSX_CELL_STYLE_INDEX_OUT_OF_RANGE",
            ],
            &[
                "TestRepairInvariantsCatchSharedStringsCountProblems",
                "TestRepairInvariantsAllowSharedStringsOmittedCountsAndReuseCount",
                "TestRepairInvariantsCatchStylesCountProblems",
                "TestRepairInvariantsCatchWorksheetStyleReferenceProblems",
                "TestRepairInvariantsCatchWorksheetStyleReferenceWithoutStyles",
            ],
        ),
        repair_class(
            "xlsx-calc-chain",
            &["xlsx"],
            &["XLSX_CALC_CHAIN_REFERENCE"],
            &[
                "TestRepairInvariantsCatchCalcChainReferenceProblems",
                "TestRepairInvariantsAllowCalcChainReferences",
            ],
        ),
        repair_class(
            "zip-metadata",
            &["pptx", "xlsx"],
            &["OOXML_ZIP_TIMESTAMP_INVALID"],
            &["TestRepairInvariantsCatchInvalidZipTimestamp"],
        ),
        repair_class(
            "local-office-open",
            &["pptx", "xlsx"],
            &["OOXML_OFFICE_CHECK_SKIPPED", "OOXML_OFFICE_CHECK_FAILED"],
            &[
                "TestConformanceOfficeOpenGoldenSummary",
                "TestCheckPackageOfficeCheckFailureFailsReport",
                "internal/cli.TestConformanceCheckOfficeCheckJSON",
                "internal/cli.TestGeneratedRepairConformanceOfficeOpenIfAvailable",
                "pkg/officecheck",
            ],
        )
        .with_status("optional-local-oracle"),
        json!({
            "id": "real-microsoft-office",
            "status": "external-oracle",
            "families": ["pptx", "xlsx"],
            "evidence": ["OFFICE_REPAIR_CONFORMANCE_GOAL.md"]
        }),
    ]
}

fn repair_class(
    id: &str,
    families: &[&str],
    diagnostic_codes: &[&str],
    evidence: &[&str],
) -> Value {
    json!({
        "id": id,
        "status": "covered",
        "families": families,
        "diagnosticCodes": diagnostic_codes,
        "evidence": evidence,
    })
}

trait CoverageStatus {
    fn with_status(self, status: &str) -> Self;
}

impl CoverageStatus for Value {
    fn with_status(mut self, status: &str) -> Self {
        self["status"] = json!(status);
        self
    }
}

fn coverage_text(value: &Value) -> String {
    let stages = value["harnessStages"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item["name"].as_str())
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    format!(
        "OOXML conformance coverage\nschemaVersion: {}\nscope: {}\nstatus: {}\nstages: {}\n",
        value["schemaVersion"].as_str().unwrap_or_default(),
        value["scope"].as_str().unwrap_or_default(),
        value["status"].as_str().unwrap_or_default(),
        stages
    )
}
