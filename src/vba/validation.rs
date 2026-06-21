use serde_json::{Value, json};
use std::collections::BTreeSet;

use crate::{
    CliResult, content_type_for_part, relationship_entries, relationships_part_for,
    resolve_relationship_target, zip_entry_exists, zip_entry_names,
};

use super::inspect::{candidate_vba_project_parts, inspect_vba_package};
use super::model::{VBA_FAMILIES, VBA_PROJECT_CONTENT_TYPE, VBA_PROJECT_REL_TYPE, VbaInfo};

pub(crate) fn vba_package_invariant_diagnostics(file: &str) -> CliResult<Vec<Value>> {
    let entries = zip_entry_names(file)?;
    let kind = crate::detect_inspect_package_type(file, &entries);
    if !VBA_FAMILIES.iter().any(|spec| spec.package_kind == kind) {
        return Ok(Vec::new());
    }
    let info = match inspect_vba_package(file) {
        Ok(info) => info,
        Err(err) => {
            return Ok(vec![vba_diagnostic(
                "VBA_PACKAGE_INSPECT_FAILED",
                "warning",
                format!("failed to inspect VBA package wiring: {}", err.message),
            )]);
        }
    };

    let mut diagnostics = Vec::new();
    let main_is_macro = info
        .main_content_type
        .eq_ignore_ascii_case(info.family.macro_content_type);
    let rels_part = relationships_part_for(&info.main_part_uri);
    let rels = relationship_entries(file, &rels_part).unwrap_or_default();
    let vba_rels = rels
        .iter()
        .filter(|rel| rel.rel_type == VBA_PROJECT_REL_TYPE)
        .collect::<Vec<_>>();
    let candidate_parts = candidate_vba_project_parts(file, &info)?;
    let candidate_set = candidate_parts.iter().cloned().collect::<BTreeSet<_>>();
    let referenced_parts = vba_rels
        .iter()
        .filter(|rel| rel.target_mode != "External")
        .map(|rel| resolve_relationship_target(&info.main_part_uri, &rel.target))
        .collect::<BTreeSet<_>>();
    let has_vba_signal = main_is_macro || !candidate_parts.is_empty() || !vba_rels.is_empty();
    if !has_vba_signal {
        push_signature_warnings(&mut diagnostics, &info);
        return Ok(diagnostics);
    }

    if vba_rels.is_empty() && !candidate_parts.is_empty() {
        diagnostics.push(vba_diagnostic(
            "VBA_PROJECT_RELATIONSHIP_MISSING",
            "error",
            format!(
                "{} has macro/VBA signals but {} does not contain a vbaProject relationship",
                info.main_part_uri, rels_part
            ),
        ));
    }
    if vba_rels.len() > 1 {
        diagnostics.push(vba_diagnostic(
            "VBA_PROJECT_RELATIONSHIP_DUPLICATE",
            "error",
            format!(
                "{} contains {} vbaProject relationships; expected exactly one",
                rels_part,
                vba_rels.len()
            ),
        ));
    }
    if (!candidate_parts.is_empty() || !vba_rels.is_empty()) && !main_is_macro {
        diagnostics.push(vba_diagnostic(
            "VBA_MAIN_PART_NOT_MACRO_ENABLED",
            "error",
            format!(
                "{} has content type {:?}, expected {:?} when a VBA project is present",
                info.main_part_uri, info.main_content_type, info.family.macro_content_type
            ),
        ));
    }
    if main_is_macro && candidate_parts.is_empty() && vba_rels.is_empty() {
        diagnostics.push(vba_diagnostic(
            "VBA_MAIN_PART_MACRO_ENABLED_WITHOUT_PROJECT",
            "warning",
            format!(
                "{} is macro-enabled but no vbaProject.bin part or relationship was found",
                info.main_part_uri
            ),
        ));
    }

    for rel in &vba_rels {
        if rel.target_mode == "External" {
            diagnostics.push(vba_diagnostic(
                "VBA_PROJECT_RELATIONSHIP_EXTERNAL",
                "error",
                format!(
                    "{} relationship {} points to an external VBA project target",
                    rels_part, rel.id
                ),
            ));
            continue;
        }
        let target_uri = resolve_relationship_target(&info.main_part_uri, &rel.target);
        if !zip_entry_exists(&entries, &target_uri) {
            diagnostics.push(vba_diagnostic(
                "VBA_PROJECT_PART_MISSING",
                "error",
                format!(
                    "{} relationship {} points to missing VBA project part {}",
                    rels_part, rel.id, target_uri
                ),
            ));
            continue;
        }
        let content_type = content_type_for_part(file, &target_uri)?;
        if !content_type.eq_ignore_ascii_case(VBA_PROJECT_CONTENT_TYPE) {
            diagnostics.push(vba_diagnostic(
                "VBA_PROJECT_CONTENT_TYPE_INVALID",
                "error",
                format!(
                    "{} has content type {:?}, expected {:?}",
                    target_uri, content_type, VBA_PROJECT_CONTENT_TYPE
                ),
            ));
        }
    }

    for part in &candidate_parts {
        let content_type = content_type_for_part(file, part)?;
        if !content_type.eq_ignore_ascii_case(VBA_PROJECT_CONTENT_TYPE) {
            diagnostics.push(vba_diagnostic(
                "VBA_PROJECT_CONTENT_TYPE_INVALID",
                "error",
                format!(
                    "{} has content type {:?}, expected {:?}",
                    part, content_type, VBA_PROJECT_CONTENT_TYPE
                ),
            ));
        }
        if !referenced_parts.contains(part) {
            let severity = if vba_rels.is_empty() {
                "error"
            } else {
                "warning"
            };
            diagnostics.push(vba_diagnostic(
                "VBA_PROJECT_ORPHAN_PART",
                severity,
                format!("{part} exists but is not targeted by a vbaProject relationship"),
            ));
        }
    }

    for part in referenced_parts.difference(&candidate_set) {
        if zip_entry_exists(&entries, part) {
            let content_type = content_type_for_part(file, part)?;
            if !content_type.eq_ignore_ascii_case(VBA_PROJECT_CONTENT_TYPE) {
                diagnostics.push(vba_diagnostic(
                    "VBA_PROJECT_CONTENT_TYPE_INVALID",
                    "error",
                    format!(
                        "{} has content type {:?}, expected {:?}",
                        part, content_type, VBA_PROJECT_CONTENT_TYPE
                    ),
                ));
            }
        }
    }

    push_signature_warnings(&mut diagnostics, &info);
    Ok(diagnostics)
}

fn push_signature_warnings(diagnostics: &mut Vec<Value>, info: &VbaInfo) {
    if info.signature_artifacts.is_empty() {
        return;
    }
    diagnostics.push(vba_diagnostic(
        "VBA_SIGNATURE_ARTIFACT_PRESENT",
        "warning",
        "known signature artifacts are present; VBA mutation commands refuse to edit signed macro packages",
    ));
}

fn vba_diagnostic(code: &str, severity: &str, message: impl Into<String>) -> Value {
    json!({
        "code": code,
        "severity": severity,
        "message": message.into(),
    })
}
