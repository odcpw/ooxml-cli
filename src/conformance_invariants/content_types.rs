use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::Value;
use std::collections::BTreeSet;

use crate::{CliResult, attr, local_name, zip_text};

use super::spec::{CONTENT_TYPES_NAMESPACE, CONTENT_TYPES_PART_URI};
use super::types::{ContentTypesInfo, PartInfo};
use super::util::{diag, element_namespace, file_extension, normalize_uri};

pub(super) fn collect_parts(entries: &[String], content_types: &ContentTypesInfo) -> Vec<PartInfo> {
    entries
        .iter()
        .filter(|entry| !entry.ends_with('/'))
        .map(|entry| {
            let uri = normalize_uri(entry);
            PartInfo {
                uri: uri.clone(),
                entry_name: entry.clone(),
                content_type: content_types.content_type_for_uri(&uri),
            }
        })
        .collect()
}

impl ContentTypesInfo {
    fn content_type_for_uri(&self, uri: &str) -> String {
        let normalized = normalize_uri(uri);
        if let Some(content_type) = self.override_types.get(&normalized) {
            return content_type.clone();
        }
        let extension = file_extension(&normalized);
        self.default_types
            .get(extension)
            .cloned()
            .unwrap_or_default()
    }
}

pub(super) fn parse_content_types(
    file: &str,
    entry_set: &BTreeSet<String>,
) -> CliResult<ContentTypesInfo> {
    if !entry_set.contains(CONTENT_TYPES_PART_URI) {
        return Ok(ContentTypesInfo::default());
    }
    let xml = match zip_text(file, "[Content_Types].xml") {
        Ok(xml) => xml,
        Err(err) => {
            let mut info = ContentTypesInfo::default();
            info.diagnostics.push(diag(
                "OOXML_CONTENT_TYPES_READ_ERROR",
                format!("failed to read {CONTENT_TYPES_PART_URI}: {}", err.message),
            ));
            return Ok(info);
        }
    };

    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);
    let mut info = ContentTypesInfo::default();
    let mut seen_root = false;
    let mut root_ok = false;
    let mut parse_ok = true;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                if !seen_root {
                    seen_root = true;
                    let actual_local = local_name(e.name().as_ref()).to_string();
                    let actual_ns = element_namespace(&e);
                    if actual_ns != CONTENT_TYPES_NAMESPACE || actual_local != "Types" {
                        info.diagnostics.push(diag(
                            "OOXML_CONTENT_TYPES_ROOT",
                            format!(
                                "{CONTENT_TYPES_PART_URI} root is {{{actual_ns}}}{actual_local}, expected {{{CONTENT_TYPES_NAMESPACE}}}Types"
                            ),
                        ));
                    } else {
                        root_ok = true;
                    }
                    continue;
                }

                match local_name(e.name().as_ref()) {
                    "Default" => {
                        let extension = attr(&e, "Extension").unwrap_or_default();
                        let content_type = attr(&e, "ContentType").unwrap_or_default();
                        let extension = extension.trim().to_string();
                        let content_type = content_type.trim().to_string();
                        if extension.is_empty() || content_type.is_empty() {
                            info.diagnostics.push(diag(
                                "OOXML_CONTENT_TYPES_DEFAULT_REQUIRED",
                                format!(
                                    "{CONTENT_TYPES_PART_URI} <Default> must have non-empty Extension and ContentType attributes"
                                ),
                            ));
                            continue;
                        }
                        if info.defaults.contains(&extension) {
                            info.diagnostics.push(diag(
                                "OOXML_CONTENT_TYPES_DEFAULT_DUPLICATE",
                                format!(
                                    "{CONTENT_TYPES_PART_URI} repeats Default Extension {extension:?}"
                                ),
                            ));
                        }
                        info.defaults.insert(extension.clone());
                        info.default_types.insert(extension, content_type);
                    }
                    "Override" => {
                        let raw_part_name = attr(&e, "PartName").unwrap_or_default();
                        let content_type = attr(&e, "ContentType").unwrap_or_default();
                        let raw_part_name = raw_part_name.trim().to_string();
                        let part_name = normalize_uri(&raw_part_name);
                        let content_type = content_type.trim().to_string();
                        if raw_part_name.is_empty()
                            || !raw_part_name.starts_with('/')
                            || part_name == "/"
                            || content_type.is_empty()
                        {
                            info.diagnostics.push(diag(
                                "OOXML_CONTENT_TYPES_OVERRIDE_REQUIRED",
                                format!(
                                    "{CONTENT_TYPES_PART_URI} <Override> must have non-empty absolute PartName and ContentType attributes"
                                ),
                            ));
                            continue;
                        }
                        if info.overrides.contains(&part_name) {
                            info.diagnostics.push(diag(
                                "OOXML_CONTENT_TYPES_OVERRIDE_DUPLICATE",
                                format!(
                                    "{CONTENT_TYPES_PART_URI} repeats Override PartName {part_name:?}"
                                ),
                            ));
                        }
                        info.overrides.insert(part_name.clone());
                        info.override_types.insert(part_name, content_type);
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => {
                info.diagnostics.push(diag(
                    "OOXML_CONTENT_TYPES_PARSE_ERROR",
                    format!("failed to parse {CONTENT_TYPES_PART_URI}: {err}"),
                ));
                parse_ok = false;
                break;
            }
            _ => {}
        }
    }

    if !seen_root {
        info.diagnostics.push(diag(
            "OOXML_CONTENT_TYPES_ROOT",
            format!("{CONTENT_TYPES_PART_URI} has no XML root"),
        ));
    }
    info.coverage_ok = parse_ok && root_ok;
    Ok(info)
}

pub(super) fn check_content_types_coverage(
    parts: &BTreeSet<String>,
    content_types: &ContentTypesInfo,
) -> Vec<Value> {
    let mut diagnostics = Vec::new();
    for part_name in &content_types.overrides {
        if !parts.contains(part_name) {
            diagnostics.push(diag(
                "OOXML_CONTENT_TYPES_OVERRIDE_TARGET_MISSING",
                format!(
                    "{CONTENT_TYPES_PART_URI} Override PartName {part_name:?} does not match a package part"
                ),
            ));
        }
    }
    for part_uri in parts {
        if part_uri == CONTENT_TYPES_PART_URI || content_types.overrides.contains(part_uri) {
            continue;
        }
        let extension = file_extension(part_uri);
        if !extension.is_empty() && content_types.defaults.contains(extension) {
            continue;
        }
        diagnostics.push(diag(
            "OOXML_CONTENT_TYPES_PART_UNMAPPED",
            format!(
                "{part_uri} has no matching Override and no Default for extension {extension:?}"
            ),
        ));
    }
    diagnostics
}
