use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::fs;

use crate::{
    CliError, CliResult, command_arg, copy_zip_with_part_overrides, ensure_content_type_override,
    ensure_package_root_relationship_xml, find_xlsx_workbook_part, local_name,
    relationship_entries, resolve_relationship_target, validate,
    validate_xlsx_mutation_output_flags, xlsx_ranges_set_temp_path, zip_entry_names, zip_text,
};

mod calc;
mod props_xml;

use calc::{update_xlsx_workbook_calc_xml, xlsx_workbook_calc_settings_from_xml};
use props_xml::{
    render_xlsx_app_props_xml, render_xlsx_core_props_xml, update_xlsx_app_props_xml,
    update_xlsx_core_props_xml, xml_direct_child_text_by_ns,
};

const XLSX_MAIN_NS: &[u8] = b"http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const XLSX_CORE_PROPS_NS: &[u8] =
    b"http://schemas.openxmlformats.org/package/2006/metadata/core-properties";
const XLSX_DUBLIN_CORE_NS: &[u8] = b"http://purl.org/dc/elements/1.1/";
const XLSX_EXTENDED_PROPS_NS: &[u8] =
    b"http://schemas.openxmlformats.org/officeDocument/2006/extended-properties";
const XLSX_CORE_PROPS_REL: &str =
    "http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties";
const XLSX_EXTENDED_PROPS_REL: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties";
const XLSX_CORE_PROPS_CONTENT_TYPE: &str =
    "application/vnd.openxmlformats-package.core-properties+xml";
const XLSX_EXTENDED_PROPS_CONTENT_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.extended-properties+xml";

#[derive(Clone, Default)]
struct XlsxWorkbookMetadataFields {
    title: String,
    subject: String,
    creator: String,
    keywords: String,
    description: String,
    last_modified_by: String,
    category: String,
    company: String,
    manager: String,
}

#[derive(Clone)]
struct XlsxWorkbookCalcSettings {
    calc_mode: String,
    full_calc_on_load: bool,
    force_full_calc: bool,
    calc_id: String,
    iterate: bool,
    iterate_count: i64,
    iterate_delta: f64,
}

impl Default for XlsxWorkbookCalcSettings {
    fn default() -> Self {
        Self {
            calc_mode: "auto".to_string(),
            full_calc_on_load: false,
            force_full_calc: false,
            calc_id: String::new(),
            iterate: false,
            iterate_count: 100,
            iterate_delta: 0.001,
        }
    }
}

#[derive(Clone, Default)]
struct XlsxWorkbookMetadataSnapshot {
    metadata: XlsxWorkbookMetadataFields,
    calc_settings: XlsxWorkbookCalcSettings,
}

pub(crate) struct XlsxWorkbookMetadataUpdateOptions<'a> {
    pub(crate) title: Option<&'a str>,
    pub(crate) subject: Option<&'a str>,
    pub(crate) creator: Option<&'a str>,
    pub(crate) keywords: Option<&'a str>,
    pub(crate) description: Option<&'a str>,
    pub(crate) last_modified_by: Option<&'a str>,
    pub(crate) category: Option<&'a str>,
    pub(crate) company: Option<&'a str>,
    pub(crate) manager: Option<&'a str>,
    pub(crate) calc_mode: Option<&'a str>,
    pub(crate) full_calc_on_load: Option<bool>,
    pub(crate) expect_title: Option<&'a str>,
    pub(crate) expect_subject: Option<&'a str>,
    pub(crate) expect_creator: Option<&'a str>,
    pub(crate) expect_keywords: Option<&'a str>,
    pub(crate) expect_description: Option<&'a str>,
    pub(crate) expect_last_modified_by: Option<&'a str>,
    pub(crate) expect_category: Option<&'a str>,
    pub(crate) expect_company: Option<&'a str>,
    pub(crate) expect_manager: Option<&'a str>,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
}

pub(crate) fn xlsx_workbook_metadata_inspect(file: &str) -> CliResult<Value> {
    let snapshot = read_xlsx_workbook_metadata(file)?;
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("action".to_string(), json!("inspect"));
    result.insert(
        "metadata".to_string(),
        xlsx_workbook_metadata_fields_json(&snapshot.metadata),
    );
    result.insert(
        "calcSettings".to_string(),
        xlsx_workbook_calc_settings_json(&snapshot.calc_settings),
    );
    result.insert(
        "inspectCommandTemplate".to_string(),
        json!("ooxml --json xlsx workbook metadata inspect <placeholder>.xlsx"),
    );
    result.insert(
        "validateCommandTemplate".to_string(),
        json!("ooxml validate <placeholder>.xlsx"),
    );
    Ok(Value::Object(result))
}

pub(crate) fn xlsx_workbook_metadata_update(
    file: &str,
    options: XlsxWorkbookMetadataUpdateOptions<'_>,
) -> CliResult<Value> {
    if !xlsx_workbook_metadata_has_updates(&options) {
        return Err(CliError::invalid_args(
            "no metadata fields specified; set at least one of --title/--subject/--creator/--keywords/--description/--last-modified-by/--category/--company/--manager/--calc-mode/--full-calc-on-load",
        ));
    }
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;

    let (current, workbook_part) = read_xlsx_workbook_metadata_with_workbook_part(file)?;
    check_xlsx_workbook_metadata_guards(&options, &current)?;

    let mut updated = current.clone();
    let mut updated_fields = Vec::<String>::new();
    let mut previous_values = Map::new();

    apply_xlsx_metadata_string_update(
        "title",
        options.title,
        &current.metadata.title,
        &mut updated.metadata.title,
        &mut updated_fields,
        &mut previous_values,
    );
    apply_xlsx_metadata_string_update(
        "subject",
        options.subject,
        &current.metadata.subject,
        &mut updated.metadata.subject,
        &mut updated_fields,
        &mut previous_values,
    );
    apply_xlsx_metadata_string_update(
        "creator",
        options.creator,
        &current.metadata.creator,
        &mut updated.metadata.creator,
        &mut updated_fields,
        &mut previous_values,
    );
    apply_xlsx_metadata_string_update(
        "description",
        options.description,
        &current.metadata.description,
        &mut updated.metadata.description,
        &mut updated_fields,
        &mut previous_values,
    );
    apply_xlsx_metadata_string_update(
        "keywords",
        options.keywords,
        &current.metadata.keywords,
        &mut updated.metadata.keywords,
        &mut updated_fields,
        &mut previous_values,
    );
    apply_xlsx_metadata_string_update(
        "lastModifiedBy",
        options.last_modified_by,
        &current.metadata.last_modified_by,
        &mut updated.metadata.last_modified_by,
        &mut updated_fields,
        &mut previous_values,
    );
    apply_xlsx_metadata_string_update(
        "category",
        options.category,
        &current.metadata.category,
        &mut updated.metadata.category,
        &mut updated_fields,
        &mut previous_values,
    );
    apply_xlsx_metadata_string_update(
        "company",
        options.company,
        &current.metadata.company,
        &mut updated.metadata.company,
        &mut updated_fields,
        &mut previous_values,
    );
    apply_xlsx_metadata_string_update(
        "manager",
        options.manager,
        &current.metadata.manager,
        &mut updated.metadata.manager,
        &mut updated_fields,
        &mut previous_values,
    );

    if let Some(calc_mode) = options.calc_mode {
        if !matches!(calc_mode, "auto" | "manual" | "autoNoTable") {
            return Err(CliError::invalid_args(format!(
                "failed to update workbook metadata: invalid calcMode {calc_mode:?} (must be auto, manual, or autoNoTable)"
            )));
        }
        updated.calc_settings.calc_mode = calc_mode.to_string();
        updated_fields.push("calcMode".to_string());
        previous_values.insert(
            "calcMode".to_string(),
            Value::String(current.calc_settings.calc_mode.clone()),
        );
    }
    if let Some(full_calc_on_load) = options.full_calc_on_load {
        updated.calc_settings.full_calc_on_load = full_calc_on_load;
        updated.calc_settings.force_full_calc = full_calc_on_load;
        updated_fields.push("fullCalcOnLoad".to_string());
        previous_values.insert(
            "fullCalcOnLoad".to_string(),
            Value::String(current.calc_settings.full_calc_on_load.to_string()),
        );
    }

    let mut overrides = BTreeMap::<String, String>::new();
    let core_uri = xlsx_metadata_part_uri(file, XLSX_CORE_PROPS_REL, "/docProps/core.xml");
    let core_part = core_uri.trim_start_matches('/').to_string();
    let app_uri = xlsx_metadata_part_uri(file, XLSX_EXTENDED_PROPS_REL, "/docProps/app.xml");
    let app_part = app_uri.trim_start_matches('/').to_string();
    let core_updates = options.title.is_some()
        || options.subject.is_some()
        || options.creator.is_some()
        || options.keywords.is_some()
        || options.description.is_some()
        || options.last_modified_by.is_some()
        || options.category.is_some();
    let app_updates = options.company.is_some() || options.manager.is_some();
    let mut created_core = false;
    let mut created_app = false;

    if core_updates {
        let existing = zip_text(file, &core_part).ok();
        created_core = existing.is_none();
        let xml = if let Some(xml) = existing {
            update_xlsx_core_props_xml(&xml, &options, &updated.metadata)
        } else {
            render_xlsx_core_props_xml(&updated.metadata)
        };
        overrides.insert(core_part.clone(), xml);
    }
    if app_updates {
        let existing = zip_text(file, &app_part).ok();
        created_app = existing.is_none();
        let xml = if let Some(xml) = existing {
            update_xlsx_app_props_xml(&xml, &options, &updated.metadata)
        } else {
            render_xlsx_app_props_xml(&updated.metadata)
        };
        overrides.insert(app_part.clone(), xml);
    }
    if options.calc_mode.is_some() || options.full_calc_on_load.is_some() {
        let workbook_xml = zip_text(file, &workbook_part).map_err(|err| {
            CliError::invalid_args(format!(
                "failed to update workbook metadata: failed to read workbook /{}: {}",
                workbook_part, err.message
            ))
        })?;
        overrides.insert(
            workbook_part.clone(),
            update_xlsx_workbook_calc_xml(
                workbook_xml,
                options.calc_mode,
                options.full_calc_on_load,
            ),
        );
    }

    if created_core || created_app {
        let mut content_types_xml = zip_text(file, "[Content_Types].xml")?;
        let mut root_rels_xml = zip_text(file, "_rels/.rels").unwrap_or_else(|_| {
            r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"></Relationships>"#.to_string()
        });
        if created_core {
            content_types_xml = ensure_content_type_override(
                content_types_xml,
                &core_uri,
                XLSX_CORE_PROPS_CONTENT_TYPE,
            );
            root_rels_xml =
                ensure_package_root_relationship_xml(root_rels_xml, XLSX_CORE_PROPS_REL, &core_uri);
        }
        if created_app {
            content_types_xml = ensure_content_type_override(
                content_types_xml,
                &app_uri,
                XLSX_EXTENDED_PROPS_CONTENT_TYPE,
            );
            root_rels_xml = ensure_package_root_relationship_xml(
                root_rels_xml,
                XLSX_EXTENDED_PROPS_REL,
                &app_uri,
            );
        }
        overrides.insert("[Content_Types].xml".to_string(), content_types_xml);
        overrides.insert("_rels/.rels".to_string(), root_rels_xml);
    }

    let output_path = options.out.filter(|value| !value.trim().is_empty());
    let commit_path = if options.in_place {
        Some(file)
    } else {
        output_path
    };
    let readback_path = if options.dry_run || options.in_place || output_path == Some(file) {
        xlsx_ranges_set_temp_path(file)
    } else {
        output_path
            .ok_or_else(|| {
                CliError::invalid_args(
                    "must specify exactly one of --out, --in-place, or --dry-run",
                )
            })?
            .to_string()
    };

    copy_zip_with_part_overrides(file, &readback_path, &overrides)?;
    if !options.no_validate {
        validate(&readback_path, true)?;
    }
    let readback = read_xlsx_workbook_metadata(&readback_path)?;
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

    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    if let Some(commit_path) = commit_path {
        result.insert("output".to_string(), json!(commit_path));
    }
    result.insert("dryRun".to_string(), json!(options.dry_run));
    result.insert("action".to_string(), json!("update"));
    result.insert(
        "metadata".to_string(),
        xlsx_workbook_metadata_fields_json(&readback.metadata),
    );
    result.insert(
        "calcSettings".to_string(),
        xlsx_workbook_calc_settings_json(&readback.calc_settings),
    );
    result.insert("updated".to_string(), json!(updated_fields.len()));
    result.insert("updatedFields".to_string(), json!(updated_fields));
    result.insert("previousValues".to_string(), Value::Object(previous_values));
    if let Some(commit_path) = commit_path {
        result.insert(
            "validateCommand".to_string(),
            json!(format!(
                "ooxml validate --strict {}",
                command_arg(commit_path)
            )),
        );
        result.insert(
            "inspectCommand".to_string(),
            json!(format!(
                "ooxml --json xlsx workbook metadata inspect {}",
                command_arg(commit_path)
            )),
        );
    }
    Ok(Value::Object(result))
}

fn read_xlsx_workbook_metadata(file: &str) -> CliResult<XlsxWorkbookMetadataSnapshot> {
    read_xlsx_workbook_metadata_with_workbook_part(file).map(|(snapshot, _)| snapshot)
}

fn read_xlsx_workbook_metadata_with_workbook_part(
    file: &str,
) -> CliResult<(XlsxWorkbookMetadataSnapshot, String)> {
    let entries = zip_entry_names(file)?;
    let workbook_part = find_xlsx_workbook_part(file, &entries)?;
    let mut snapshot = XlsxWorkbookMetadataSnapshot::default();

    let core_uri = xlsx_metadata_part_uri(file, XLSX_CORE_PROPS_REL, "/docProps/core.xml");
    if let Ok(xml) = zip_text(file, core_uri.trim_start_matches('/')) {
        snapshot.metadata.title = xml_direct_child_text_by_ns(&xml, XLSX_DUBLIN_CORE_NS, "title");
        snapshot.metadata.subject =
            xml_direct_child_text_by_ns(&xml, XLSX_DUBLIN_CORE_NS, "subject");
        snapshot.metadata.creator =
            xml_direct_child_text_by_ns(&xml, XLSX_DUBLIN_CORE_NS, "creator");
        snapshot.metadata.description =
            xml_direct_child_text_by_ns(&xml, XLSX_DUBLIN_CORE_NS, "description");
        snapshot.metadata.keywords =
            xml_direct_child_text_by_ns(&xml, XLSX_CORE_PROPS_NS, "keywords");
        snapshot.metadata.last_modified_by =
            xml_direct_child_text_by_ns(&xml, XLSX_CORE_PROPS_NS, "lastModifiedBy");
        snapshot.metadata.category =
            xml_direct_child_text_by_ns(&xml, XLSX_CORE_PROPS_NS, "category");
    }

    let app_uri = xlsx_metadata_part_uri(file, XLSX_EXTENDED_PROPS_REL, "/docProps/app.xml");
    if let Ok(xml) = zip_text(file, app_uri.trim_start_matches('/')) {
        snapshot.metadata.company =
            xml_direct_child_text_by_ns(&xml, XLSX_EXTENDED_PROPS_NS, "Company");
        snapshot.metadata.manager =
            xml_direct_child_text_by_ns(&xml, XLSX_EXTENDED_PROPS_NS, "Manager");
    }

    if let Ok(workbook_xml) = zip_text(file, &workbook_part) {
        snapshot.calc_settings = xlsx_workbook_calc_settings_from_xml(&workbook_xml);
    }

    Ok((snapshot, workbook_part))
}

fn xlsx_metadata_part_uri(file: &str, rel_type: &str, fallback: &str) -> String {
    for rel in relationship_entries(file, "_rels/.rels").unwrap_or_default() {
        if rel.target_mode == "External" {
            continue;
        }
        if rel.rel_type == rel_type {
            return resolve_relationship_target("/", &rel.target);
        }
    }
    fallback.to_string()
}

fn xlsx_workbook_metadata_has_updates(options: &XlsxWorkbookMetadataUpdateOptions<'_>) -> bool {
    options.title.is_some()
        || options.subject.is_some()
        || options.creator.is_some()
        || options.keywords.is_some()
        || options.description.is_some()
        || options.last_modified_by.is_some()
        || options.category.is_some()
        || options.company.is_some()
        || options.manager.is_some()
        || options.calc_mode.is_some()
        || options.full_calc_on_load.is_some()
}

fn check_xlsx_workbook_metadata_guards(
    options: &XlsxWorkbookMetadataUpdateOptions<'_>,
    current: &XlsxWorkbookMetadataSnapshot,
) -> CliResult<()> {
    let checks = [
        ("title", options.expect_title, &current.metadata.title),
        ("subject", options.expect_subject, &current.metadata.subject),
        ("creator", options.expect_creator, &current.metadata.creator),
        (
            "keywords",
            options.expect_keywords,
            &current.metadata.keywords,
        ),
        (
            "description",
            options.expect_description,
            &current.metadata.description,
        ),
        (
            "lastModifiedBy",
            options.expect_last_modified_by,
            &current.metadata.last_modified_by,
        ),
        (
            "category",
            options.expect_category,
            &current.metadata.category,
        ),
        ("company", options.expect_company, &current.metadata.company),
        ("manager", options.expect_manager, &current.metadata.manager),
    ];
    for (name, want, got) in checks {
        if let Some(want) = want
            && got != want
        {
            return Err(CliError::invalid_args(format!(
                "failed to update workbook metadata: expected {name} to be {want:?} but found {got:?}"
            )));
        }
    }
    Ok(())
}

fn apply_xlsx_metadata_string_update(
    name: &str,
    value: Option<&str>,
    previous: &str,
    target: &mut String,
    updated_fields: &mut Vec<String>,
    previous_values: &mut Map<String, Value>,
) {
    if let Some(value) = value {
        *target = value.to_string();
        updated_fields.push(name.to_string());
        previous_values.insert(name.to_string(), Value::String(previous.to_string()));
    }
}

fn xlsx_workbook_metadata_fields_json(fields: &XlsxWorkbookMetadataFields) -> Value {
    json!({
        "title": fields.title,
        "subject": fields.subject,
        "creator": fields.creator,
        "keywords": fields.keywords,
        "description": fields.description,
        "lastModifiedBy": fields.last_modified_by,
        "category": fields.category,
        "company": fields.company,
        "manager": fields.manager,
    })
}

fn xlsx_workbook_calc_settings_json(calc: &XlsxWorkbookCalcSettings) -> Value {
    json!({
        "calcMode": calc.calc_mode,
        "fullCalcOnLoad": calc.full_calc_on_load,
        "forceFullCalc": calc.force_full_calc,
        "calcId": calc.calc_id,
        "iterate": calc.iterate,
        "iterateCount": calc.iterate_count,
        "iterateDelta": calc.iterate_delta,
    })
}

fn metadata_ordered_insert_position(
    xml: &str,
    target_order: i32,
    order: fn(&str) -> i32,
) -> Option<usize> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut depth = 0usize;
    loop {
        let start = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                if depth == 1 && order(local_name(e.name().as_ref())) > target_order {
                    return Some(start);
                }
                depth += 1;
            }
            Ok(Event::Empty(e)) => {
                if depth == 1 && order(local_name(e.name().as_ref())) > target_order {
                    return Some(start);
                }
            }
            Ok(Event::End(_)) => {
                if depth == 1 {
                    return Some(start);
                }
                depth = depth.saturating_sub(1);
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    None
}
