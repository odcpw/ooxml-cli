use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

use crate::{
    CliError, CliResult, RelationshipEntry, content_type_for_part, relationship_entries,
    relationships_part_for, resolve_relationship_target, zip_bytes, zip_entry_exists,
    zip_entry_names,
};

use super::model::{
    SignatureArtifact, VBA_FAMILIES, VBA_PROJECT_CONTENT_TYPE, VBA_PROJECT_REL_TYPE, VbaFamilySpec,
    VbaInfo, VbaProjectInfo,
};
use super::package_xml::package_part_name;

pub(super) fn inspect_vba_package(file: &str) -> CliResult<VbaInfo> {
    let entries = zip_entry_names(file)?;
    let family = detect_vba_family(file, &entries)?;
    let main_part_uri = find_vba_main_part(file, &entries, family)?;
    let main_content_type = content_type_for_part(file, &main_part_uri)?;
    let project = inspect_vba_project(file, &entries, &main_part_uri, family)?;
    let signature_artifacts = find_signature_artifacts(file, &entries)?;
    let has_project = project.as_ref().is_some_and(|project| project.exists);
    let macro_enabled = main_content_type.eq_ignore_ascii_case(family.macro_content_type)
        || has_project
        || project
            .as_ref()
            .is_some_and(|project| !project.relationship_id.is_empty());
    let mut warnings = Vec::new();
    if main_content_type.eq_ignore_ascii_case(family.macro_content_type) && !has_project {
        warnings.push("main part is macro-enabled but no VBA project part was found".to_string());
    }
    if !main_content_type.eq_ignore_ascii_case(family.macro_content_type) && has_project {
        warnings.push("VBA project exists but main content type is not macro-enabled".to_string());
    }
    if !signature_artifacts.is_empty() {
        warnings.push(
            "known signature artifacts are present; attach/remove refuses to mutate signed macro packages"
                .to_string(),
        );
    }
    Ok(VbaInfo {
        family,
        package_type: family.family,
        macro_enabled,
        main_part_uri,
        main_content_type,
        project,
        signature_artifacts,
        warnings,
    })
}

fn detect_vba_family(file: &str, entries: &[String]) -> CliResult<&'static VbaFamilySpec> {
    let kind = crate::detect_inspect_package_type(file, entries);
    VBA_FAMILIES
        .iter()
        .find(|spec| spec.package_kind == kind)
        .ok_or_else(|| {
            let detected = crate::package_type(file).unwrap_or("unknown");
            CliError::unsupported_type(format!(
                "VBA package operations support PPTX/PPTM, DOCX/DOCM, and XLSX/XLSM (detected: {detected})"
            ))
        })
}

fn find_vba_main_part(file: &str, entries: &[String], spec: &VbaFamilySpec) -> CliResult<String> {
    for rel in relationship_entries(file, "_rels/.rels").unwrap_or_default() {
        if rel.target_mode == "External" {
            continue;
        }
        let target = resolve_relationship_target("/", &rel.target);
        let content_type = content_type_for_part(file, &target).unwrap_or_default();
        if target == spec.default_main_part
            || content_type.eq_ignore_ascii_case(spec.non_macro_content_type)
            || content_type.eq_ignore_ascii_case(spec.macro_content_type)
        {
            return Ok(target);
        }
    }
    if zip_entry_exists(entries, spec.default_main_part) {
        return Ok(spec.default_main_part.to_string());
    }
    Err(CliError::unexpected(format!(
        "could not locate {} main part",
        spec.family
    )))
}

fn inspect_vba_project(
    file: &str,
    entries: &[String],
    main_part_uri: &str,
    spec: &VbaFamilySpec,
) -> CliResult<Option<VbaProjectInfo>> {
    let rels_part = relationships_part_for(main_part_uri);
    let rels = relationship_entries(file, &rels_part).unwrap_or_default();
    let mut rel_match = None::<RelationshipEntry>;
    let mut part_uri = String::new();
    for rel in rels {
        let target = resolve_relationship_target(main_part_uri, &rel.target);
        let content_type = content_type_for_part(file, &target).unwrap_or_default();
        if rel.rel_type == VBA_PROJECT_REL_TYPE
            || target == spec.default_vba_part
            || content_type.eq_ignore_ascii_case(VBA_PROJECT_CONTENT_TYPE)
        {
            part_uri = target;
            rel_match = Some(rel);
            break;
        }
    }
    if part_uri.is_empty() {
        part_uri = first_vba_project_part(file, entries, spec)?.unwrap_or_default();
    }
    if part_uri.is_empty() {
        return Ok(None);
    }
    let part_name = package_part_name(&part_uri);
    let exists = zip_entry_exists(entries, &part_uri);
    let content_type = content_type_for_part(file, &part_uri).unwrap_or_default();
    let (size_bytes, sha256) = if exists {
        let data = zip_bytes(file, &part_name)?;
        let mut hasher = Sha256::new();
        hasher.update(&data);
        (Some(data.len()), Some(format!("{:x}", hasher.finalize())))
    } else {
        (None, None)
    };
    let rel = rel_match.unwrap_or(RelationshipEntry {
        id: String::new(),
        rel_type: String::new(),
        target: String::new(),
        target_mode: String::new(),
    });
    Ok(Some(VbaProjectInfo {
        part_uri,
        content_type,
        exists,
        size_bytes,
        sha256,
        relationship_id: rel.id,
        relationship_type: rel.rel_type,
        relationship_target: rel.target,
    }))
}

fn first_vba_project_part(
    file: &str,
    entries: &[String],
    spec: &VbaFamilySpec,
) -> CliResult<Option<String>> {
    if zip_entry_exists(entries, spec.default_vba_part) {
        return Ok(Some(spec.default_vba_part.to_string()));
    }
    for entry in entries {
        let uri = format!("/{entry}");
        if content_type_for_part(file, &uri)?.eq_ignore_ascii_case(VBA_PROJECT_CONTENT_TYPE) {
            return Ok(Some(uri));
        }
    }
    Ok(None)
}

pub(super) fn candidate_vba_project_parts(file: &str, info: &VbaInfo) -> CliResult<Vec<String>> {
    let entries = zip_entry_names(file)?;
    let mut candidates = Vec::new();
    candidates.push(info.family.default_vba_part.to_string());
    if let Some(project) = &info.project {
        candidates.push(project.part_uri.clone());
    }
    for entry in &entries {
        let uri = format!("/{entry}");
        if content_type_for_part(file, &uri)?.eq_ignore_ascii_case(VBA_PROJECT_CONTENT_TYPE) {
            candidates.push(uri);
        }
    }
    let mut seen = BTreeSet::new();
    Ok(candidates
        .into_iter()
        .filter(|part| zip_entry_exists(&entries, part))
        .filter(|part| seen.insert(package_part_name(part)))
        .collect())
}

fn find_signature_artifacts(file: &str, entries: &[String]) -> CliResult<Vec<SignatureArtifact>> {
    let mut artifacts = Vec::new();
    let mut seen = BTreeSet::new();
    let mut add = |artifact: SignatureArtifact| {
        let key = format!(
            "{}|{}|{}|{}|{}",
            artifact.kind,
            artifact.part_uri,
            artifact.source_uri,
            artifact.relationship_id,
            artifact.target
        );
        if seen.insert(key) {
            artifacts.push(artifact);
        }
    };

    let mut sources = vec!["/".to_string()];
    for entry in entries {
        let uri = format!("/{entry}");
        sources.push(uri.clone());
        let lower_uri = uri.to_ascii_lowercase();
        let lower_content_type = content_type_for_part(file, &uri)
            .unwrap_or_default()
            .to_ascii_lowercase();
        if lower_uri.contains("_xmlsignatures")
            || lower_uri.contains("vbaprojectsignature")
            || lower_content_type.contains("digital-signature")
            || lower_content_type.contains("vbaprojectsignature")
        {
            add(SignatureArtifact {
                kind: "part".to_string(),
                part_uri: uri,
                source_uri: String::new(),
                relationship_id: String::new(),
                rel_type: String::new(),
                target: String::new(),
            });
        }
    }

    for source_uri in sources {
        let rels_part = if source_uri == "/" {
            "_rels/.rels".to_string()
        } else {
            relationships_part_for(&source_uri)
        };
        for rel in relationship_entries(file, &rels_part).unwrap_or_default() {
            let lower_type = rel.rel_type.to_ascii_lowercase();
            if lower_type.contains("digital-signature")
                || lower_type.contains("vbaprojectsignature")
            {
                add(SignatureArtifact {
                    kind: "relationship".to_string(),
                    part_uri: resolve_relationship_target(&source_uri, &rel.target),
                    source_uri: source_uri.clone(),
                    relationship_id: rel.id,
                    rel_type: rel.rel_type,
                    target: rel.target,
                });
            }
        }
    }

    Ok(artifacts)
}
