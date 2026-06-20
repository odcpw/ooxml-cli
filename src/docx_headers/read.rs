use serde_json::{Value, json};
use std::collections::BTreeMap;

use super::paragraphs::docx_header_footer_paragraphs;
use super::sections::docx_header_footer_sections;
use super::selectors::{
    DocxHeaderFooterSelector, normalize_docx_header_footer_show_type,
    parse_docx_header_footer_selector, resolve_docx_header_footer_selector,
};
use crate::{
    CliError, CliResult, InspectPackageKind, detect_inspect_package_type, find_docx_document_part,
    has_flag, package_type, parse_i64_flag, parse_string_flag, reject_unknown_flags,
    relationship_entries, relationships_part_for, resolve_relationship_target, zip_entry_names,
    zip_text,
};

pub(crate) fn docx_headers_footers_list(file: &str) -> CliResult<Value> {
    let (document_uri, sections) = docx_header_footer_listing(file)?;
    Ok(json!({
        "file": file,
        "documentPartUri": document_uri,
        "sections": sections,
    }))
}

pub(super) fn docx_header_footer_listing(file: &str) -> CliResult<(String, Vec<Value>)> {
    let entries = zip_entry_names(file)?;
    let package_kind = detect_inspect_package_type(file, &entries);
    if package_kind != InspectPackageKind::Docx {
        let detected = match package_kind {
            InspectPackageKind::Pptx => "pptx",
            InspectPackageKind::Xlsx => "xlsx",
            InspectPackageKind::Docx => "docx",
            InspectPackageKind::Unknown => package_type(file)?,
        };
        return Err(CliError::unsupported_type(format!(
            "file is not a DOCX document (detected: {detected})"
        )));
    }

    let document_part = find_docx_document_part(file, &entries)?;
    let document_uri = format!("/{}", document_part.trim_start_matches('/'));
    let xml = zip_text(file, &document_part).map_err(|err| {
        CliError::unexpected(format!(
            "failed to list headers/footers: failed to read document part {document_uri}: {}",
            err.message
        ))
    })?;
    let rel_targets = relationship_entries(file, &relationships_part_for(&document_part))
        .unwrap_or_default()
        .into_iter()
        .filter(|rel| rel.target_mode != "External")
        .map(|rel| {
            (
                rel.id,
                resolve_relationship_target(&document_uri, &rel.target),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let sections = docx_header_footer_sections(file, &xml, &rel_targets)?;
    Ok((document_uri, sections))
}

pub(crate) fn docx_headers_footers_show(
    file: &str,
    kind: &str,
    rest: &[String],
) -> CliResult<Value> {
    reject_unknown_flags(rest, &["--id", "--type", "--section", "--selector"], &[])?;
    let id = parse_string_flag(rest, "--id")?.unwrap_or_default();
    let ref_type = parse_string_flag(rest, "--type")?.unwrap_or_else(|| "default".to_string());
    let ref_type = normalize_docx_header_footer_show_type(&ref_type)?;
    let section = parse_i64_flag(rest, "--section")?.unwrap_or(0);
    if section < 0 {
        return Err(CliError::invalid_args(
            "--section must be >= 0 (0 means the last section)",
        ));
    }
    let selector = parse_string_flag(rest, "--selector")?;
    if selector.is_some()
        && (has_flag(rest, "--id") || has_flag(rest, "--type") || has_flag(rest, "--section"))
    {
        return Err(CliError::invalid_args(
            "cannot specify --selector with --id, --type, or --section",
        ));
    }

    let (_document_uri, sections) = docx_header_footer_listing(file)?;
    let target = if let Some(selector) = selector {
        let parsed = parse_docx_header_footer_selector(kind, &selector)?;
        resolve_docx_header_footer_selector(&sections, kind, &parsed)
    } else if !id.is_empty() {
        resolve_docx_header_footer_selector(
            &sections,
            kind,
            &DocxHeaderFooterSelector {
                kind: kind.to_string(),
                id,
                ref_type,
                section,
                ..DocxHeaderFooterSelector::default()
            },
        )
    } else {
        resolve_docx_header_footer_selector(
            &sections,
            kind,
            &DocxHeaderFooterSelector {
                kind: kind.to_string(),
                ref_type,
                section,
                ..DocxHeaderFooterSelector::default()
            },
        )
    }
    .ok_or_else(|| CliError::target_not_found(format!("target not found: {kind}")))?;

    if target.part_uri.is_empty() {
        return Err(CliError::invalid_args(format!(
            "{kind} reference {:?} does not resolve to a part",
            target.id
        )));
    }
    let paragraphs = docx_header_footer_paragraphs(file, &target)?;
    Ok(json!({
        "file": file,
        "kind": target.kind,
        "partUri": target.part_uri,
        "id": target.id,
        "type": target.ref_type,
        "section": target.section,
        "primarySelector": target.primary_selector,
        "selectors": target.selectors,
        "paragraphs": paragraphs,
    }))
}
