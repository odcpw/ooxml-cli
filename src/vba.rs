use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::{
    CliError, CliResult, InspectPackageKind, RelationshipEntry, add_relationship_to_xml,
    allocate_relationship_id, attr, command_arg, content_type_for_part,
    copy_zip_with_binary_part_overrides_and_removals, local_name, package_mutation_temp_path,
    relationship_entries, relationship_target_from_source_to_target, relationships_part_for,
    resolve_relationship_target, validate, validate_xlsx_mutation_output_flags, zip_bytes,
    zip_entry_exists, zip_entry_names, zip_text,
};

const VBA_PROJECT_CONTENT_TYPE: &str = "application/vnd.ms-office.vbaProject";
const VBA_PROJECT_REL_TYPE: &str =
    "http://schemas.microsoft.com/office/2006/relationships/vbaProject";

struct VbaFamilySpec {
    family: &'static str,
    package_kind: InspectPackageKind,
    default_main_part: &'static str,
    default_vba_part: &'static str,
    non_macro_content_type: &'static str,
    macro_content_type: &'static str,
    non_macro_extension: &'static str,
    macro_extension: &'static str,
}

const VBA_FAMILIES: &[VbaFamilySpec] = &[
    VbaFamilySpec {
        family: "pptx",
        package_kind: InspectPackageKind::Pptx,
        default_main_part: "/ppt/presentation.xml",
        default_vba_part: "/ppt/vbaProject.bin",
        non_macro_content_type: "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml",
        macro_content_type: "application/vnd.ms-powerpoint.presentation.macroEnabled.main+xml",
        non_macro_extension: ".pptx",
        macro_extension: ".pptm",
    },
    VbaFamilySpec {
        family: "xlsx",
        package_kind: InspectPackageKind::Xlsx,
        default_main_part: "/xl/workbook.xml",
        default_vba_part: "/xl/vbaProject.bin",
        non_macro_content_type: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml",
        macro_content_type: "application/vnd.ms-excel.sheet.macroEnabled.main+xml",
        non_macro_extension: ".xlsx",
        macro_extension: ".xlsm",
    },
];

pub(crate) struct VbaMutationOptions<'a> {
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
}

struct VbaInfo {
    family: &'static VbaFamilySpec,
    package_type: &'static str,
    macro_enabled: bool,
    main_part_uri: String,
    main_content_type: String,
    project: Option<VbaProjectInfo>,
    warnings: Vec<String>,
}

struct VbaProjectInfo {
    part_uri: String,
    content_type: String,
    exists: bool,
    size_bytes: Option<usize>,
    sha256: Option<String>,
    relationship_id: String,
    relationship_type: String,
    relationship_target: String,
}

pub(crate) fn vba_inspect(file: &str) -> CliResult<Value> {
    let info = inspect_vba_package(file)?;
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("vba".to_string(), vba_info_json(&info));
    result.insert(
        "validateCommand".to_string(),
        json!(vba_validate_command(file)),
    );
    if let Some(command) = vba_office_check_command(file, &info) {
        result.insert("officeCheckCommand".to_string(), json!(command));
    }
    result.insert(
        "packageReadbackCommand".to_string(),
        json!(vba_package_readback_command(file, info.family.family)),
    );
    if let Some(command) = vba_extract_bin_command(file, &info) {
        result.insert("extractBinCommand".to_string(), json!(command));
    }
    result.insert(
        "nextMutationTemplate".to_string(),
        json!(vba_next_mutation_template(file, &info)),
    );
    Ok(Value::Object(result))
}

pub(crate) fn vba_extract_bin(file: &str, out: &str) -> CliResult<Value> {
    if out.trim().is_empty() {
        return Err(CliError::invalid_args("--out is required"));
    }
    let info = inspect_vba_package(file)?;
    let project = info
        .project
        .as_ref()
        .filter(|project| project.exists)
        .ok_or_else(|| CliError::target_not_found("package has no vbaProject.bin part"))?;
    let part_name = package_part_name(&project.part_uri);
    let data = zip_bytes(file, &part_name)?;
    if let Some(parent) = Path::new(out).parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|err| {
            CliError::unexpected(format!("failed to create output directory: {err}"))
        })?;
    }
    fs::write(out, &data)
        .map_err(|err| CliError::unexpected(format!("failed to write VBA binary: {err}")))?;

    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("output".to_string(), json!(out));
    result.insert("bytesWritten".to_string(), json!(data.len()));
    result.insert("vba".to_string(), vba_info_json(&info));
    result.insert(
        "inspectCommand".to_string(),
        json!(vba_inspect_command(file)),
    );
    result.insert(
        "validateCommand".to_string(),
        json!(vba_validate_command(file)),
    );
    if let Some(command) = vba_office_check_command(file, &info) {
        result.insert("officeCheckCommand".to_string(), json!(command));
    }
    result.insert(
        "packageReadbackCommand".to_string(),
        json!(vba_package_readback_command(file, info.family.family)),
    );
    result.insert(
        "attachCommandTemplate".to_string(),
        json!(vba_attach_template_for_bin(out, &info)),
    );
    Ok(Value::Object(result))
}

pub(crate) fn vba_attach(
    file: &str,
    bin_path: &str,
    options: VbaMutationOptions<'_>,
) -> CliResult<Value> {
    if bin_path.trim().is_empty() {
        return Err(CliError::invalid_args("--bin is required"));
    }
    let data = fs::read(bin_path)
        .map_err(|err| CliError::file_not_found(format!("failed to read VBA binary: {err}")))?;
    if data.is_empty() {
        return Err(CliError::invalid_args("vbaProject.bin is empty"));
    }
    let info = inspect_vba_package(file)?;
    let target_part = info
        .project
        .as_ref()
        .map(|project| project.part_uri.clone())
        .filter(|part| !part.trim().is_empty())
        .unwrap_or_else(|| info.family.default_vba_part.to_string());
    let main_part = package_part_name(&info.main_part_uri);
    let main_data = zip_bytes(file, &main_part)?;
    let mut text_overrides = BTreeMap::new();
    let mut binary_overrides = BTreeMap::new();
    let mut removals = BTreeSet::new();
    let content_types = set_content_type_override(
        &set_content_type_override(
            &zip_text(file, "[Content_Types].xml")?,
            &info.main_part_uri,
            info.family.macro_content_type,
        ),
        &target_part,
        VBA_PROJECT_CONTENT_TYPE,
    );
    let rels_part = relationships_part_for(&info.main_part_uri);
    let rels_xml = zip_text(file, &rels_part).unwrap_or_else(|_| {
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"></Relationships>"#.to_string()
    });
    text_overrides.insert("[Content_Types].xml".to_string(), content_types);
    text_overrides.insert(
        rels_part,
        upsert_vba_relationship_xml(&rels_xml, file, &info, &target_part),
    );
    binary_overrides.insert(main_part, main_data);
    binary_overrides.insert(package_part_name(&target_part), data);

    let result = write_vba_mutation(
        file,
        options,
        &text_overrides,
        &binary_overrides,
        &mut removals,
        inspect_vba_package,
        |output_info| vba_mutation_result_json("attach", output_info, Some(&target_part), true),
    )?;
    Ok(result)
}

pub(crate) fn vba_remove(file: &str, options: VbaMutationOptions<'_>) -> CliResult<Value> {
    let info = inspect_vba_package(file)?;
    let main_part = package_part_name(&info.main_part_uri);
    let main_data = zip_bytes(file, &main_part)?;
    let mut text_overrides = BTreeMap::new();
    let mut binary_overrides = BTreeMap::new();
    let candidate_parts = candidate_vba_project_parts(file, &info)?;
    let mut removals = candidate_parts
        .into_iter()
        .flat_map(|part| {
            let mut parts = vec![package_part_name(&part)];
            parts.push(relationships_part_for(&part));
            parts
        })
        .collect::<BTreeSet<_>>();
    let removed_part = removals
        .iter()
        .find(|part| part.ends_with("vbaProject.bin"))
        .map(|part| format!("/{part}"));
    let mut content_types = set_content_type_override(
        &zip_text(file, "[Content_Types].xml")?,
        &info.main_part_uri,
        info.family.non_macro_content_type,
    );
    for part in &removals {
        if part.ends_with("vbaProject.bin") {
            content_types = remove_content_type_override(&content_types, part);
        }
    }
    let rels_part = relationships_part_for(&info.main_part_uri);
    let rels_xml = zip_text(file, &rels_part).unwrap_or_else(|_| {
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"></Relationships>"#.to_string()
    });
    text_overrides.insert("[Content_Types].xml".to_string(), content_types);
    text_overrides.insert(
        rels_part,
        remove_vba_relationships_xml(&rels_xml, file, &info),
    );
    binary_overrides.insert(main_part, main_data);

    let result = write_vba_mutation(
        file,
        options,
        &text_overrides,
        &binary_overrides,
        &mut removals,
        inspect_vba_package,
        |output_info| {
            vba_mutation_result_json(
                "remove",
                output_info,
                removed_part.as_deref().or_else(|| {
                    info.project
                        .as_ref()
                        .map(|project| project.part_uri.as_str())
                }),
                false,
            )
        },
    )?;
    Ok(result)
}

fn write_vba_mutation<F, G>(
    file: &str,
    options: VbaMutationOptions<'_>,
    text_overrides: &BTreeMap<String, String>,
    binary_overrides: &BTreeMap<String, Vec<u8>>,
    removals: &mut BTreeSet<String>,
    readback: F,
    mutation_result: G,
) -> CliResult<Value>
where
    F: Fn(&str) -> CliResult<VbaInfo>,
    G: Fn(&VbaInfo) -> Value,
{
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;
    let output_path = options.out.filter(|value| !value.trim().is_empty());
    let commit_path = if options.in_place {
        Some(file)
    } else {
        output_path
    };
    let readback_path = if options.dry_run || options.in_place || output_path == Some(file) {
        package_mutation_temp_path(file, "vba-mutation")
    } else {
        output_path
            .ok_or_else(|| {
                CliError::invalid_args(
                    "must specify exactly one of --out, --in-place, or --dry-run",
                )
            })?
            .to_string()
    };

    for key in text_overrides.keys().chain(binary_overrides.keys()) {
        removals.remove(key);
    }
    copy_zip_with_binary_part_overrides_and_removals(
        file,
        &readback_path,
        text_overrides,
        binary_overrides,
        removals,
    )?;
    if !options.no_validate {
        validate(&readback_path, true)?;
    }
    let output_info = readback(&readback_path)?;

    if options.dry_run {
        let _ = fs::remove_file(&readback_path);
    } else if options.in_place || output_path == Some(file) {
        if let Some(backup_path) = options.backup.filter(|value| !value.trim().is_empty()) {
            fs::copy(file, backup_path)
                .map_err(|err| CliError::unexpected(format!("failed to create backup: {err}")))?;
        }
        fs::rename(&readback_path, file)
            .or_else(|_| {
                fs::copy(&readback_path, file)?;
                fs::remove_file(&readback_path)
            })
            .map_err(|err| CliError::unexpected(format!("failed to write output file: {err}")))?;
    }

    let target = if options.dry_run {
        vba_output_placeholder(if output_info.macro_enabled {
            output_info.family.macro_extension
        } else {
            output_info.family.non_macro_extension
        })
    } else {
        commit_path.unwrap_or(&readback_path).to_string()
    };
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    if !options.dry_run
        && let Some(commit_path) = commit_path
    {
        result.insert("output".to_string(), json!(commit_path));
    }
    if options.dry_run {
        result.insert("dryRun".to_string(), json!(true));
    }
    result.insert("result".to_string(), mutation_result(&output_info));
    result.insert("vba".to_string(), vba_info_json(&output_info));
    if options.dry_run {
        result.insert(
            "inspectCommandTemplate".to_string(),
            json!(vba_inspect_command(&target)),
        );
        result.insert(
            "validateCommandTemplate".to_string(),
            json!(vba_validate_command(&target)),
        );
        if let Some(command) = vba_office_check_command(&target, &output_info) {
            result.insert("officeCheckCommandTemplate".to_string(), json!(command));
        }
        result.insert(
            "packageReadbackCommandTemplate".to_string(),
            json!(vba_package_readback_command(
                &target,
                output_info.family.family
            )),
        );
    } else {
        result.insert(
            "inspectCommand".to_string(),
            json!(vba_inspect_command(&target)),
        );
        result.insert(
            "validateCommand".to_string(),
            json!(vba_validate_command(&target)),
        );
        if let Some(command) = vba_office_check_command(&target, &output_info) {
            result.insert("officeCheckCommand".to_string(), json!(command));
        }
        result.insert(
            "packageReadbackCommand".to_string(),
            json!(vba_package_readback_command(
                &target,
                output_info.family.family
            )),
        );
        if let Some(command) = vba_extract_bin_command(&target, &output_info) {
            result.insert("extractBinCommand".to_string(), json!(command));
        }
        result.insert(
            "nextMutationTemplate".to_string(),
            json!(vba_next_mutation_template(&target, &output_info)),
        );
    }
    Ok(Value::Object(result))
}

fn inspect_vba_package(file: &str) -> CliResult<VbaInfo> {
    let entries = zip_entry_names(file)?;
    let family = detect_vba_family(file, &entries)?;
    let main_part_uri = find_vba_main_part(file, &entries, family)?;
    let main_content_type = content_type_for_part(file, &main_part_uri)?;
    let project = inspect_vba_project(file, &entries, &main_part_uri, family)?;
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
    Ok(VbaInfo {
        family,
        package_type: family.family,
        macro_enabled,
        main_part_uri,
        main_content_type,
        project,
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
                "VBA package operations support PPTX/PPTM and XLSX/XLSM only (detected: {detected})"
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

fn candidate_vba_project_parts(file: &str, info: &VbaInfo) -> CliResult<Vec<String>> {
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

fn upsert_vba_relationship_xml(
    xml: &str,
    file: &str,
    info: &VbaInfo,
    project_part_uri: &str,
) -> String {
    let rels = relationship_entries_from_optional_xml(xml);
    let target = relationship_target_from_source_to_target(&info.main_part_uri, project_part_uri);
    let mut updated = false;
    let out = rewrite_relationships_xml(xml, |rel| {
        let target_uri = resolve_relationship_target(&info.main_part_uri, &rel.target);
        let content_type = content_type_for_part(file, &target_uri).unwrap_or_default();
        if rel.rel_type == VBA_PROJECT_REL_TYPE
            || target_uri == project_part_uri
            || content_type.eq_ignore_ascii_case(VBA_PROJECT_CONTENT_TYPE)
        {
            updated = true;
            Some(relationship_xml(&rel.id, VBA_PROJECT_REL_TYPE, &target))
        } else {
            Some(relationship_xml(&rel.id, &rel.rel_type, &rel.target))
        }
    });
    if updated {
        return out;
    }
    add_relationship_to_xml(
        out,
        &allocate_relationship_id(&rels),
        VBA_PROJECT_REL_TYPE,
        &target,
    )
}

fn remove_vba_relationships_xml(xml: &str, file: &str, info: &VbaInfo) -> String {
    rewrite_relationships_xml(xml, |rel| {
        let target_uri = resolve_relationship_target(&info.main_part_uri, &rel.target);
        let content_type = content_type_for_part(file, &target_uri).unwrap_or_default();
        if rel.rel_type == VBA_PROJECT_REL_TYPE
            || content_type.eq_ignore_ascii_case(VBA_PROJECT_CONTENT_TYPE)
        {
            None
        } else {
            Some(relationship_xml(&rel.id, &rel.rel_type, &rel.target))
        }
    })
}

fn rewrite_relationships_xml<F>(xml: &str, mut mapper: F) -> String
where
    F: FnMut(&RelationshipEntry) -> Option<String>,
{
    let rels = relationship_entries_from_optional_xml(xml);
    let mut body = String::new();
    for rel in &rels {
        if let Some(rendered) = mapper(rel) {
            body.push_str(&rendered);
        }
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{body}</Relationships>"#
    )
}

fn relationship_entries_from_optional_xml(xml: &str) -> Vec<RelationshipEntry> {
    crate::relationship_entries_from_xml(xml)
}

fn relationship_xml(id: &str, rel_type: &str, target: &str) -> String {
    format!(
        r#"<Relationship Id="{}" Type="{}" Target="{}"/>"#,
        crate::xml_attr_escape(id),
        crate::xml_attr_escape(rel_type),
        crate::xml_attr_escape(target)
    )
}

fn set_content_type_override(xml: &str, part_name: &str, content_type: &str) -> String {
    let normalized = format!("/{}", part_name.trim_start_matches('/'));
    let replacement = format!(
        r#"<Override PartName="{}" ContentType="{}"/>"#,
        crate::xml_attr_escape(&normalized),
        crate::xml_attr_escape(content_type)
    );
    if let Some((start, end)) = content_type_override_span(xml, &normalized) {
        let mut out = String::with_capacity(xml.len() + replacement.len());
        out.push_str(&xml[..start]);
        out.push_str(&replacement);
        out.push_str(&xml[end..]);
        return out;
    }
    if let Some(pos) = xml.rfind("</Types>") {
        let mut out = String::with_capacity(xml.len() + replacement.len());
        out.push_str(&xml[..pos]);
        out.push_str(&replacement);
        out.push_str(&xml[pos..]);
        return out;
    }
    xml.to_string()
}

fn remove_content_type_override(xml: &str, part_name: &str) -> String {
    let normalized = format!("/{}", part_name.trim_start_matches('/'));
    let Some((start, end)) = content_type_override_span(xml, &normalized) else {
        return xml.to_string();
    };
    let mut out = String::with_capacity(xml.len());
    out.push_str(&xml[..start]);
    out.push_str(&xml[end..]);
    out
}

fn content_type_override_span(xml: &str, part_name: &str) -> Option<(usize, usize)> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Empty(e)) | Ok(Event::Start(e))
                if local_name(e.name().as_ref()) == "Override"
                    && attr(&e, "PartName").is_some_and(|value| value == part_name) =>
            {
                return Some((before, reader.buffer_position() as usize));
            }
            Ok(Event::Eof) | Err(_) => return None,
            _ => {}
        }
    }
}

fn vba_info_json(info: &VbaInfo) -> Value {
    let mut object = Map::new();
    object.insert("family".to_string(), json!(info.family.family));
    object.insert("packageType".to_string(), json!(info.package_type));
    object.insert("macroEnabled".to_string(), json!(info.macro_enabled));
    object.insert(
        "hasVbaProject".to_string(),
        json!(info.project.as_ref().is_some_and(|project| project.exists)),
    );
    object.insert("mainPartUri".to_string(), json!(info.main_part_uri));
    object.insert("mainContentType".to_string(), json!(info.main_content_type));
    if let Some(project) = &info.project {
        object.insert("vbaProject".to_string(), vba_project_json(project));
    }
    object.insert(
        "nonMacroExtension".to_string(),
        json!(info.family.non_macro_extension),
    );
    object.insert(
        "macroExtension".to_string(),
        json!(info.family.macro_extension),
    );
    if !info.warnings.is_empty() {
        object.insert("warnings".to_string(), json!(info.warnings));
    }
    Value::Object(object)
}

fn vba_project_json(project: &VbaProjectInfo) -> Value {
    let mut object = Map::new();
    object.insert("partUri".to_string(), json!(project.part_uri));
    object.insert("contentType".to_string(), json!(project.content_type));
    object.insert("exists".to_string(), json!(project.exists));
    if let Some(size) = project.size_bytes {
        object.insert("sizeBytes".to_string(), json!(size));
    }
    if let Some(sha256) = &project.sha256 {
        object.insert("sha256".to_string(), json!(sha256));
    }
    if !project.relationship_id.is_empty() {
        object.insert("relationshipId".to_string(), json!(project.relationship_id));
    }
    if !project.relationship_type.is_empty() {
        object.insert(
            "relationshipType".to_string(),
            json!(project.relationship_type),
        );
    }
    if !project.relationship_target.is_empty() {
        object.insert(
            "relationshipTarget".to_string(),
            json!(project.relationship_target),
        );
    }
    Value::Object(object)
}

fn vba_mutation_result_json(
    action: &str,
    info: &VbaInfo,
    vba_part_uri: Option<&str>,
    macro_enabled: bool,
) -> Value {
    let mut object = Map::new();
    object.insert("action".to_string(), json!(action));
    object.insert("family".to_string(), json!(info.family.family));
    object.insert("mainPartUri".to_string(), json!(info.main_part_uri));
    if let Some(vba_part_uri) = vba_part_uri.filter(|part| !part.trim().is_empty()) {
        object.insert("vbaPartUri".to_string(), json!(vba_part_uri));
    }
    object.insert("macroEnabled".to_string(), json!(macro_enabled));
    Value::Object(object)
}

fn vba_inspect_command(file: &str) -> String {
    format!("ooxml --json vba inspect {}", command_arg(file))
}

fn vba_validate_command(file: &str) -> String {
    format!("ooxml validate --strict {}", command_arg(file))
}

fn vba_office_check_command(file: &str, info: &VbaInfo) -> Option<String> {
    if !info.macro_enabled && !info.project.as_ref().is_some_and(|project| project.exists) {
        return None;
    }
    Some(format!(
        "ooxml --json vba office-check {}",
        command_arg(file)
    ))
}

fn vba_extract_bin_command(file: &str, info: &VbaInfo) -> Option<String> {
    if !info.project.as_ref().is_some_and(|project| project.exists) {
        return None;
    }
    Some(format!(
        "ooxml --json vba extract-bin {} --out vbaProject.bin",
        command_arg(file)
    ))
}

fn vba_package_readback_command(file: &str, family: &str) -> String {
    match family {
        "pptx" => format!("ooxml --json pptx slides list {}", command_arg(file)),
        "xlsx" => format!("ooxml --json xlsx sheets list {}", command_arg(file)),
        _ => String::new(),
    }
}

fn vba_next_mutation_template(file: &str, info: &VbaInfo) -> String {
    if info.macro_enabled || info.project.as_ref().is_some_and(|project| project.exists) {
        return format!(
            "ooxml --json vba remove {} --out {}",
            command_arg(file),
            command_arg(&vba_output_placeholder(info.family.non_macro_extension))
        );
    }
    format!(
        "ooxml --json vba attach {} --bin vbaProject.bin --out {}",
        command_arg(file),
        command_arg(&vba_output_placeholder(info.family.macro_extension))
    )
}

fn vba_attach_template_for_bin(bin_path: &str, info: &VbaInfo) -> String {
    format!(
        "ooxml --json vba attach {} --bin {} --out {}",
        command_arg(&vba_output_placeholder(info.family.non_macro_extension)),
        command_arg(bin_path),
        command_arg(&vba_output_placeholder(info.family.macro_extension))
    )
}

fn vba_output_placeholder(extension: &str) -> String {
    if extension.is_empty() {
        "<out>".to_string()
    } else {
        format!("<out{extension}>")
    }
}

fn package_part_name(uri: &str) -> String {
    uri.trim_start_matches('/').to_string()
}
