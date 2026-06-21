use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Map, Value, json};
use std::collections::BTreeSet;

use crate::{
    CliResult, EXIT_PARTIAL_SUCCESS, EXIT_SUCCESS, EXIT_VALIDATION_FAILED, InspectPackageKind,
    detect_inspect_package_type, find_docx_document_part, find_xlsx_workbook_part, local_name,
    relationship_entries, relationship_source_uri, relationships, relationships_part_for,
    resolve_relationship_target, workbook_sheets, zip_entry_names, zip_entry_set, zip_text,
};

pub(crate) fn validate(file: &str, strict: bool) -> CliResult<Value> {
    let diagnostics = validate_diagnostics(file)?;
    Ok(validate_report(file, diagnostics, strict))
}

pub(crate) fn validate_exit_code(report: &Value, strict: bool) -> i32 {
    let errors = report
        .get("summary")
        .and_then(|summary| summary.get("errors"))
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let warnings = report
        .get("summary")
        .and_then(|summary| summary.get("warnings"))
        .and_then(Value::as_i64)
        .unwrap_or(0);
    if errors > 0 || (strict && warnings > 0) {
        EXIT_VALIDATION_FAILED
    } else if warnings > 0 {
        EXIT_PARTIAL_SUCCESS
    } else {
        EXIT_SUCCESS
    }
}

fn validate_report(file: &str, diagnostics: Vec<Value>, strict: bool) -> Value {
    let mut errors = 0usize;
    let mut warnings = 0usize;
    let mut info = 0usize;
    for diagnostic in &diagnostics {
        match diagnostic
            .get("severity")
            .and_then(Value::as_str)
            .unwrap_or_default()
        {
            "error" => errors += 1,
            "warning" => warnings += 1,
            "info" => info += 1,
            _ => {}
        }
    }
    let exit_code = if errors > 0 || (strict && warnings > 0) {
        EXIT_VALIDATION_FAILED
    } else if warnings > 0 {
        EXIT_PARTIAL_SUCCESS
    } else {
        EXIT_SUCCESS
    };
    let status = if exit_code == EXIT_VALIDATION_FAILED {
        "errors"
    } else if exit_code == EXIT_PARTIAL_SUCCESS {
        "warnings"
    } else {
        "valid"
    };
    let mut report = Map::new();
    report.insert("file".to_string(), json!(file));
    report.insert("valid".to_string(), json!(exit_code == EXIT_SUCCESS));
    report.insert("status".to_string(), json!(status));
    if !diagnostics.is_empty() {
        report.insert("diagnostics".to_string(), Value::Array(diagnostics));
    }
    report.insert(
        "summary".to_string(),
        json!({"errors": errors, "warnings": warnings, "info": info}),
    );
    Value::Object(report)
}

fn validate_diagnostics(file: &str) -> CliResult<Vec<Value>> {
    let entries = zip_entry_names(file)?;
    let entry_set = zip_entry_set(&entries);
    let mut diagnostics = Vec::new();
    if !entries.iter().any(|name| name == "[Content_Types].xml") {
        diagnostics.push(validation_diagnostic(
            "PKG_MISSING_CONTENT_TYPES",
            "error",
            "[Content_Types].xml not found in package root",
        ));
    }
    if !entries.iter().any(|name| name == "_rels/.rels") {
        diagnostics.push(validation_diagnostic(
            "PKG_MISSING_PACKAGE_RELS",
            "error",
            "/_rels/.rels not found in package",
        ));
    }

    diagnostics.extend(validate_relationship_integrity(file, &entries, &entry_set)?);

    match detect_inspect_package_type(file, &entries) {
        InspectPackageKind::Docx => {
            diagnostics.extend(validate_docx_required_parts(file, &entries, &entry_set)?);
        }
        InspectPackageKind::Xlsx => {
            diagnostics.extend(validate_xlsx_required_parts(file, &entries, &entry_set)?);
        }
        InspectPackageKind::Pptx => {
            diagnostics.extend(crate::validation_pptx::validate_pptx_semantics(
                file, &entry_set,
            )?);
        }
        InspectPackageKind::Unknown => {}
    }

    Ok(diagnostics)
}

fn validation_diagnostic(code: &str, severity: &str, message: impl Into<String>) -> Value {
    json!({
        "code": code,
        "severity": severity,
        "message": message.into(),
    })
}

fn validate_relationship_integrity(
    file: &str,
    entries: &[String],
    entry_set: &BTreeSet<String>,
) -> CliResult<Vec<Value>> {
    let mut diagnostics = Vec::new();
    for rels_part in entries.iter().filter(|entry| entry.ends_with(".rels")) {
        let source_uri = relationship_source_uri(rels_part);
        for rel in relationship_entries(file, rels_part)? {
            if rel.target_mode == "External" {
                continue;
            }
            let target_uri = resolve_relationship_target(&source_uri, &rel.target);
            if !entry_set.contains(&target_uri) {
                let message = if source_uri == "/" {
                    format!(
                        "package-level relationship (id={}) points to missing part: {}",
                        rel.id, target_uri
                    )
                } else {
                    format!(
                        "relationship from {} (id={}) points to missing part: {}",
                        source_uri, rel.id, target_uri
                    )
                };
                diagnostics.push(validation_diagnostic(
                    "REL_DANGLING_TARGET",
                    "error",
                    message,
                ));
            }
        }
    }
    Ok(diagnostics)
}

fn validate_docx_required_parts(
    file: &str,
    entries: &[String],
    entry_set: &BTreeSet<String>,
) -> CliResult<Vec<Value>> {
    let mut diagnostics = Vec::new();
    match find_docx_document_part(file, entries) {
        Ok(document_part) => {
            let document_uri = format!("/{}", document_part.trim_start_matches('/'));
            if !entry_set.contains(&document_uri) {
                diagnostics.push(validation_diagnostic(
                    "DOCX_MISSING_DOCUMENT",
                    "error",
                    format!("main document part not found: {document_uri}"),
                ));
            }
        }
        Err(err) => diagnostics.push(validation_diagnostic(
            "DOCX_PARSE_ERROR",
            "error",
            format!("failed to find main document part: {}", err.message),
        )),
    }
    Ok(diagnostics)
}

fn validate_xlsx_required_parts(
    file: &str,
    entries: &[String],
    entry_set: &BTreeSet<String>,
) -> CliResult<Vec<Value>> {
    let mut diagnostics = Vec::new();
    let workbook_part = match find_xlsx_workbook_part(file, entries) {
        Ok(workbook_part) => workbook_part,
        Err(err) => {
            diagnostics.push(validation_diagnostic(
                "XLSX_PARSE_ERROR",
                "error",
                format!("failed to parse workbook structure: {}", err.message),
            ));
            return Ok(diagnostics);
        }
    };
    let workbook_uri = format!("/{}", workbook_part.trim_start_matches('/'));
    if !entry_set.contains(&workbook_uri) {
        diagnostics.push(validation_diagnostic(
            "XLSX_MISSING_WORKBOOK",
            "error",
            format!("workbook part not found: {workbook_uri}"),
        ));
        return Ok(diagnostics);
    }

    let workbook = zip_text(file, &workbook_part)?;
    diagnostics.extend(validate_xlsx_workbook_child_order(&workbook_uri, &workbook));
    let sheets = match workbook_sheets(&workbook) {
        Ok(sheets) => sheets,
        Err(err) => {
            diagnostics.push(validation_diagnostic(
                "XLSX_PARSE_ERROR",
                "error",
                format!("failed to parse workbook structure: {}", err.message),
            ));
            return Ok(diagnostics);
        }
    };
    if sheets.is_empty() {
        diagnostics.push(validation_diagnostic(
            "XLSX_NO_SHEETS",
            "error",
            "workbook contains no sheets",
        ));
        return Ok(diagnostics);
    }

    let rels_part = relationships_part_for(&workbook_part);
    let rels = relationships(file, &rels_part).unwrap_or_default();
    for sheet in sheets {
        let Some(target) = rels.get(&sheet.rel_id) else {
            diagnostics.push(validation_diagnostic(
                "XLSX_SHEET_RELATIONSHIP_NOT_FOUND",
                "error",
                format!(
                    "sheet {} ({:?}) relationship {} not found in workbook rels",
                    sheet.position, sheet.name, sheet.rel_id
                ),
            ));
            continue;
        };
        let target_uri = resolve_relationship_target(&workbook_uri, target);
        if !entry_set.contains(&target_uri) {
            diagnostics.push(validation_diagnostic(
                "XLSX_MISSING_WORKSHEET",
                "error",
                format!(
                    "sheet {} ({:?}) points to missing worksheet part: {}",
                    sheet.position, sheet.name, target_uri
                ),
            ));
        }
    }
    Ok(diagnostics)
}

fn validate_xlsx_workbook_child_order(workbook_uri: &str, workbook_xml: &str) -> Vec<Value> {
    let mut reader = Reader::from_str(workbook_xml);
    reader.config_mut().trim_text(false);
    let mut diagnostics = Vec::new();
    let mut depth = 0_u32;
    let mut last_order = 0usize;
    let mut last_name = String::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if depth == 1 {
                    push_xlsx_workbook_child_order_diagnostic(
                        workbook_uri,
                        &name,
                        &last_name,
                        &mut last_order,
                        &mut diagnostics,
                    );
                    last_name = name;
                }
                depth += 1;
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if depth == 1 {
                    push_xlsx_workbook_child_order_diagnostic(
                        workbook_uri,
                        &name,
                        &last_name,
                        &mut last_order,
                        &mut diagnostics,
                    );
                    last_name = name;
                }
            }
            Ok(Event::End(_)) => {
                depth = depth.saturating_sub(1);
            }
            Ok(Event::Eof) => break,
            Err(err) => {
                diagnostics.push(validation_diagnostic(
                    "XLSX_PARSE_ERROR",
                    "error",
                    format!("failed to parse workbook XML child order: {err}"),
                ));
                break;
            }
            _ => {}
        }
    }

    diagnostics
}

fn push_xlsx_workbook_child_order_diagnostic(
    workbook_uri: &str,
    name: &str,
    last_name: &str,
    last_order: &mut usize,
    diagnostics: &mut Vec<Value>,
) {
    let current = xlsx_workbook_child_order(name);
    if current == 0 {
        return;
    }
    if *last_order > current {
        diagnostics.push(validation_diagnostic(
            "XLSX_WORKBOOK_CHILD_ORDER",
            "error",
            format!("{workbook_uri} has <{name}> after <{last_name}>; expected schema child order"),
        ));
        return;
    }
    *last_order = current;
}

fn xlsx_workbook_child_order(name: &str) -> usize {
    [
        "fileVersion",
        "fileSharing",
        "workbookPr",
        "workbookProtection",
        "bookViews",
        "sheets",
        "functionGroups",
        "externalReferences",
        "definedNames",
        "calcPr",
        "oleSize",
        "customWorkbookViews",
        "pivotCaches",
        "smartTagPr",
        "smartTagTypes",
        "webPublishing",
        "fileRecoveryPr",
        "webPublishObjects",
        "extLst",
    ]
    .iter()
    .position(|candidate| *candidate == name)
    .map(|idx| idx + 1)
    .unwrap_or(0)
}
