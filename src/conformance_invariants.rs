use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

use crate::xml_util::decode_local_xml_attrs;
use crate::{
    CliResult, attr, attr_exact, decode_xml_text, local_name, relationship_source_uri,
    relationships_part_for, resolve_relationship_target, zip_entry_names, zip_entry_set, zip_text,
};

const CONTENT_TYPES_PART_URI: &str = "/[Content_Types].xml";
const CONTENT_TYPES_NAMESPACE: &str =
    "http://schemas.openxmlformats.org/package/2006/content-types";
const SPREADSHEETML_NAMESPACE: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const PRESENTATIONML_NAMESPACE: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
const WORDPROCESSINGML_NAMESPACE: &str =
    "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const SPREADSHEET_DRAWING_NAMESPACE: &str =
    "http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing";
const CHART_NAMESPACE: &str = "http://schemas.openxmlformats.org/drawingml/2006/chart";

const CONTENT_TYPE_RELS: &str = "application/vnd.openxmlformats-package.relationships+xml";
const CONTENT_TYPE_XLSX_WORKBOOK: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml";
const CONTENT_TYPE_XLSX_WORKBOOK_MACRO: &str =
    "application/vnd.ms-excel.sheet.macroEnabled.main+xml";
const CONTENT_TYPE_XLSX_WORKBOOK_TEMPLATE: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.template.main+xml";
const CONTENT_TYPE_XLSX_WORKBOOK_ADDIN: &str =
    "application/vnd.ms-excel.addin.macroEnabled.main+xml";
const CONTENT_TYPE_XLSX_SHARED_STRINGS: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml";
const CONTENT_TYPE_XLSX_STYLES: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml";
const CONTENT_TYPE_XLSX_CALC_CHAIN: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.calcChain+xml";
const CONTENT_TYPE_XLSX_WORKSHEET: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml";
const CONTENT_TYPE_XLSX_CHARTSHEET: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.chartsheet+xml";
const CONTENT_TYPE_XLSX_DIALOGSHEET: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.dialogsheet+xml";
const CONTENT_TYPE_XLSX_TABLE: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.table+xml";
const CONTENT_TYPE_XLSX_PIVOT_TABLE: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.pivotTable+xml";
const CONTENT_TYPE_XLSX_PIVOT_CACHE: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.pivotCacheDefinition+xml";
const CONTENT_TYPE_XLSX_PIVOT_RECORDS: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.pivotCacheRecords+xml";
const CONTENT_TYPE_XLSX_COMMENTS: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.comments+xml";
const CONTENT_TYPE_XLSX_VML: &str = "application/vnd.openxmlformats-officedocument.vmlDrawing";
const CONTENT_TYPE_DRAWING: &str = "application/vnd.openxmlformats-officedocument.drawing+xml";
const CONTENT_TYPE_CHART: &str =
    "application/vnd.openxmlformats-officedocument.drawingml.chart+xml";

const CONTENT_TYPE_PPTX_PRESENTATION: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml";
const CONTENT_TYPE_PPTX_PRESENTATION_MACRO: &str =
    "application/vnd.ms-powerpoint.presentation.macroEnabled.main+xml";
const CONTENT_TYPE_PPTX_TEMPLATE: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.template.main+xml";
const CONTENT_TYPE_PPTX_TEMPLATE_MACRO: &str =
    "application/vnd.ms-powerpoint.template.macroEnabled.main+xml";
const CONTENT_TYPE_PPTX_SLIDESHOW: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.slideshow.main+xml";
const CONTENT_TYPE_PPTX_SLIDESHOW_MACRO: &str =
    "application/vnd.ms-powerpoint.slideshow.macroEnabled.main+xml";
const CONTENT_TYPE_PPTX_SLIDE: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.slide+xml";
const CONTENT_TYPE_PPTX_SLIDE_LAYOUT: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml";
const CONTENT_TYPE_PPTX_SLIDE_MASTER: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml";
const CONTENT_TYPE_PPTX_NOTES_SLIDE: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.notesSlide+xml";
const CONTENT_TYPE_PPTX_NOTES_MASTER: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.notesMaster+xml";
const CONTENT_TYPE_PPTX_THEME: &str = "application/vnd.openxmlformats-officedocument.theme+xml";
const CONTENT_TYPE_PPTX_TABLE_STYLES: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.tableStyles+xml";
const CONTENT_TYPE_PPTX_COMMENTS: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.comments+xml";
const CONTENT_TYPE_PPTX_COMMENT_AUTHORS: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.commentAuthors+xml";
const CONTENT_TYPE_PPTX_PRES_PROPS: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.presProps+xml";
const CONTENT_TYPE_PPTX_VIEW_PROPS: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.viewProps+xml";

const CONTENT_TYPE_DOCX_DOCUMENT: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml";
const CONTENT_TYPE_DOCX_DOCUMENT_MACRO: &str =
    "application/vnd.ms-word.document.macroEnabled.main+xml";
const CONTENT_TYPE_DOCX_TEMPLATE: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.template.main+xml";
const CONTENT_TYPE_DOCX_TEMPLATE_MACRO: &str =
    "application/vnd.ms-word.template.macroEnabledTemplate.main+xml";
const CONTENT_TYPE_DOCX_STYLES: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml";
const CONTENT_TYPE_DOCX_NUMBERING: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml";
const CONTENT_TYPE_DOCX_FOOTNOTES: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml";
const CONTENT_TYPE_DOCX_ENDNOTES: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.endnotes+xml";
const CONTENT_TYPE_DOCX_COMMENTS: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml";
const CONTENT_TYPE_DOCX_HEADER: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml";
const CONTENT_TYPE_DOCX_FOOTER: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml";

const REL_TYPE_OFFICE_DOCUMENT: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument";
const REL_TYPE_XLSX_WORKSHEET: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet";
const REL_TYPE_XLSX_CHARTSHEET: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/chartsheet";
const REL_TYPE_XLSX_DIALOGSHEET: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/dialogsheet";
const REL_TYPE_XLSX_SHARED_STRINGS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings";
const REL_TYPE_STYLES: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles";
const REL_TYPE_DOCX_NUMBERING: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering";
const REL_TYPE_DOCX_HEADER: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/header";
const REL_TYPE_DOCX_FOOTER: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer";
const REL_TYPE_DOCX_FOOTNOTES: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes";
const REL_TYPE_DOCX_ENDNOTES: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/endnotes";
const REL_TYPE_XLSX_CALC_CHAIN: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/calcChain";
const REL_TYPE_XLSX_TABLE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/table";
const REL_TYPE_XLSX_DRAWING: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/drawing";
const REL_TYPE_CHART: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/chart";
const REL_TYPE_XLSX_PIVOT_TABLE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotTable";
const REL_TYPE_XLSX_PIVOT_CACHE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotCacheDefinition";
const REL_TYPE_XLSX_PIVOT_RECORDS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotCacheRecords";
const REL_TYPE_COMMENTS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments";
const REL_TYPE_PPTX_SLIDE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide";
const REL_TYPE_PPTX_SLIDE_LAYOUT: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout";
const REL_TYPE_PPTX_SLIDE_MASTER: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster";
const REL_TYPE_PPTX_NOTES_SLIDE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesSlide";
const REL_TYPE_PPTX_NOTES_MASTER: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesMaster";
const REL_TYPE_OFFICE_THEME: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme";
const REL_TYPE_PPTX_COMMENT_AUTHORS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/commentAuthors";

#[derive(Clone)]
struct PartInfo {
    uri: String,
    entry_name: String,
    content_type: String,
}

#[derive(Clone, Default)]
struct ContentTypesInfo {
    defaults: BTreeSet<String>,
    overrides: BTreeSet<String>,
    default_types: BTreeMap<String, String>,
    override_types: BTreeMap<String, String>,
    diagnostics: Vec<Value>,
    coverage_ok: bool,
}

#[derive(Clone, Default)]
struct RelationshipRecord {
    id: String,
    rel_type: String,
    target: String,
    target_mode: String,
}

#[derive(Clone, Default)]
struct XmlElementInfo {
    local_name: String,
    namespace: String,
    attrs: BTreeMap<String, String>,
}

#[derive(Clone, Default)]
struct XmlPartInfo {
    root: Option<XmlElementInfo>,
    children: Vec<XmlElementInfo>,
    direct_child_counts: BTreeMap<(String, String), usize>,
}

pub(crate) fn check_repair_invariants(file: &str) -> CliResult<Vec<Value>> {
    let entries = zip_entry_names(file)?;
    let entry_set = zip_entry_set(&entries);
    let content_types = parse_content_types(file, &entry_set)?;
    let parts = collect_parts(&entries, &content_types);

    let mut diagnostics = Vec::new();
    diagnostics.extend(content_types.diagnostics.clone());
    if content_types.coverage_ok {
        diagnostics.extend(check_content_types_coverage(&entry_set, &content_types));
    }
    diagnostics.extend(check_package_relationship_closure(
        file, &entries, &entry_set, &parts,
    )?);

    for part in &parts {
        diagnostics.extend(check_known_part_content_type(&part.uri, &part.content_type));
        if is_rels_uri(&part.uri) {
            match parse_relationship_part(file, &part.entry_name) {
                Ok(_) => {}
                Err(err) => diagnostics.push(diag(
                    "OOXML_RELS_PARSE_ERROR",
                    format!("failed to parse relationships part {}: {err}", part.uri),
                )),
            }
            continue;
        }
        diagnostics.extend(check_part_xml_invariants(file, part)?);
    }

    Ok(diagnostics)
}

fn collect_parts(entries: &[String], content_types: &ContentTypesInfo) -> Vec<PartInfo> {
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

fn parse_content_types(file: &str, entry_set: &BTreeSet<String>) -> CliResult<ContentTypesInfo> {
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

fn check_content_types_coverage(
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

fn check_package_relationship_closure(
    file: &str,
    entries: &[String],
    entry_set: &BTreeSet<String>,
    parts: &[PartInfo],
) -> CliResult<Vec<Value>> {
    let mut diagnostics = Vec::new();
    let mut relationship_sources = BTreeSet::from(["/".to_string()]);
    let content_types: BTreeMap<String, String> = parts
        .iter()
        .map(|part| (part.uri.clone(), part.content_type.clone()))
        .collect();

    for part in parts {
        if is_rels_uri(&part.uri) {
            let source_uri = relationship_source_uri(&part.entry_name);
            relationship_sources.insert(source_uri.clone());
            if source_uri != "/" && !entry_set.contains(&source_uri) {
                diagnostics.push(diag(
                    "OOXML_RELS_ORPHANED",
                    format!(
                        "{} is a relationships part for missing source part {source_uri}",
                        part.uri
                    ),
                ));
            }
        } else {
            relationship_sources.insert(part.uri.clone());
        }
    }

    for source_uri in relationship_sources {
        let rels_part = rels_part_for_source(&source_uri);
        if !entries.iter().any(|entry| entry == &rels_part) {
            continue;
        }
        let rels = match parse_relationship_part(file, &rels_part) {
            Ok(rels) => rels,
            Err(_) => continue,
        };
        let mut seen_ids = BTreeSet::new();
        for rel in rels {
            let label = relationship_label(&source_uri, &rel);
            let trimmed_id = rel.id.trim();
            if trimmed_id.is_empty() {
                diagnostics.push(diag(
                    "OOXML_RELATIONSHIP_MISSING_ID",
                    format!("{label} is missing Id"),
                ));
            } else if seen_ids.contains(trimmed_id) {
                diagnostics.push(diag(
                    "OOXML_RELATIONSHIP_DUPLICATE_ID",
                    format!("{label} duplicates Id {}", rel.id),
                ));
            }
            seen_ids.insert(rel.id.clone());

            if rel.rel_type.trim().is_empty() {
                diagnostics.push(diag(
                    "OOXML_RELATIONSHIP_MISSING_TYPE",
                    format!("{label} is missing Type"),
                ));
            }
            if rel.target.trim().is_empty() {
                diagnostics.push(diag(
                    "OOXML_RELATIONSHIP_MISSING_TARGET",
                    format!("{label} is missing Target"),
                ));
                continue;
            }
            if !rel.target_mode.is_empty() && rel.target_mode != "External" {
                diagnostics.push(diag(
                    "OOXML_RELATIONSHIP_TARGET_MODE",
                    format!("{label} has unsupported TargetMode {:?}", rel.target_mode),
                ));
            }
            if rel.target_mode == "External" {
                continue;
            }
            if looks_external_relationship_target(&rel.target) {
                diagnostics.push(diag(
                    "OOXML_RELATIONSHIP_EXTERNAL_MODE_MISSING",
                    format!(
                        "{label} target {:?} looks external but TargetMode is not External",
                        rel.target
                    ),
                ));
                continue;
            }
            let target_uri = normalize_uri(&resolve_relationship_target(&source_uri, &rel.target));
            if !entry_set.contains(&target_uri) {
                diagnostics.push(diag(
                    "OOXML_RELATIONSHIP_TARGET_MISSING",
                    format!("{label} points to missing part {target_uri}"),
                ));
                continue;
            }
            let expected =
                expected_relationship_target_content_types(&source_uri, &target_uri, &rel.rel_type);
            if !expected.is_empty() {
                let actual = content_types.get(&target_uri).cloned().unwrap_or_default();
                if !expected.contains(&actual.as_str()) {
                    diagnostics.push(diag(
                        "OOXML_RELATIONSHIP_TARGET_CONTENT_TYPE",
                        format!(
                            "{label} has type {:?} but target {target_uri} has content type {:?}; expected one of: {}",
                            rel.rel_type,
                            actual,
                            expected.join(", ")
                        ),
                    ));
                }
            }
        }
    }

    Ok(diagnostics)
}

fn check_known_part_content_type(part_uri: &str, content_type: &str) -> Vec<Value> {
    let expected = expected_content_types_for_part(part_uri);
    if expected.is_empty() || expected.contains(&content_type) {
        return Vec::new();
    }
    vec![diag(
        "OOXML_CONTENT_TYPE_MISMATCH",
        format!(
            "{part_uri} has content type {content_type:?}, expected one of: {}",
            expected.join(", ")
        ),
    )]
}

fn check_part_xml_invariants(file: &str, part: &PartInfo) -> CliResult<Vec<Value>> {
    let check = match part.content_type.as_str() {
        ct if is_xlsx_workbook_content_type(ct) => Some((
            "workbook",
            "workbook",
            SPREADSHEETML_NAMESPACE,
            "XLSX_WORKBOOK_ROOT",
            None,
        )),
        CONTENT_TYPE_XLSX_SHARED_STRINGS => Some((
            "shared strings",
            "sst",
            SPREADSHEETML_NAMESPACE,
            "XLSX_SHARED_STRINGS_ROOT",
            None,
        )),
        CONTENT_TYPE_XLSX_STYLES => Some((
            "styles",
            "styleSheet",
            SPREADSHEETML_NAMESPACE,
            "XLSX_STYLES_ROOT",
            None,
        )),
        CONTENT_TYPE_XLSX_CALC_CHAIN => Some((
            "calc chain",
            "calcChain",
            SPREADSHEETML_NAMESPACE,
            "XLSX_CALC_CHAIN_ROOT",
            None,
        )),
        CONTENT_TYPE_XLSX_TABLE => Some((
            "table",
            "table",
            SPREADSHEETML_NAMESPACE,
            "XLSX_TABLE_ROOT",
            None,
        )),
        CONTENT_TYPE_XLSX_PIVOT_TABLE => Some((
            "pivot table",
            "pivotTableDefinition",
            SPREADSHEETML_NAMESPACE,
            "XLSX_PIVOT_TABLE_ROOT",
            None,
        )),
        CONTENT_TYPE_XLSX_PIVOT_CACHE => Some((
            "pivot cache definition",
            "pivotCacheDefinition",
            SPREADSHEETML_NAMESPACE,
            "XLSX_PIVOT_CACHE_ROOT",
            None,
        )),
        CONTENT_TYPE_XLSX_PIVOT_RECORDS => Some((
            "pivot cache records",
            "pivotCacheRecords",
            SPREADSHEETML_NAMESPACE,
            "XLSX_PIVOT_RECORDS_ROOT",
            None,
        )),
        ct if is_pptx_presentation_content_type(ct) => Some((
            "presentation",
            "presentation",
            PRESENTATIONML_NAMESPACE,
            "PPTX_PRESENTATION_ROOT",
            None,
        )),
        CONTENT_TYPE_DOCX_DOCUMENT => Some((
            "document",
            "document",
            WORDPROCESSINGML_NAMESPACE,
            "DOCX_DOCUMENT_ROOT",
            None,
        )),
        CONTENT_TYPE_DOCX_HEADER => Some((
            "header",
            "hdr",
            WORDPROCESSINGML_NAMESPACE,
            "DOCX_HEADER_ROOT",
            None,
        )),
        CONTENT_TYPE_DOCX_FOOTER => Some((
            "footer",
            "ftr",
            WORDPROCESSINGML_NAMESPACE,
            "DOCX_FOOTER_ROOT",
            None,
        )),
        CONTENT_TYPE_XLSX_WORKSHEET => Some((
            "worksheet",
            "worksheet",
            SPREADSHEETML_NAMESPACE,
            "XLSX_WORKSHEET_ROOT",
            Some((
                "XLSX_WORKSHEET_CHILD_ORDER",
                worksheet_child_order as fn(&str) -> usize,
            )),
        )),
        CONTENT_TYPE_PPTX_SLIDE => Some((
            "slide",
            "sld",
            PRESENTATIONML_NAMESPACE,
            "PPTX_SLIDE_ROOT",
            Some((
                "PPTX_SLIDE_CHILD_ORDER",
                slide_child_order as fn(&str) -> usize,
            )),
        )),
        CONTENT_TYPE_PPTX_SLIDE_LAYOUT => Some((
            "slide layout",
            "sldLayout",
            PRESENTATIONML_NAMESPACE,
            "PPTX_SLIDE_LAYOUT_ROOT",
            Some((
                "PPTX_SLIDE_LAYOUT_CHILD_ORDER",
                slide_layout_child_order as fn(&str) -> usize,
            )),
        )),
        CONTENT_TYPE_PPTX_SLIDE_MASTER => Some((
            "slide master",
            "sldMaster",
            PRESENTATIONML_NAMESPACE,
            "PPTX_SLIDE_MASTER_ROOT",
            Some((
                "PPTX_SLIDE_MASTER_CHILD_ORDER",
                slide_master_child_order as fn(&str) -> usize,
            )),
        )),
        CONTENT_TYPE_DRAWING => Some((
            "drawing",
            "wsDr",
            SPREADSHEET_DRAWING_NAMESPACE,
            "XLSX_DRAWING_ROOT",
            None,
        )),
        CONTENT_TYPE_CHART => Some((
            "chart",
            "chartSpace",
            CHART_NAMESPACE,
            "OOXML_CHART_ROOT",
            None,
        )),
        _ => None,
    };

    let Some((label, expected_local, expected_ns, root_code, order_check)) = check else {
        return Ok(Vec::new());
    };
    let info = match read_xml_part_info(file, part) {
        Ok(info) => info,
        Err(err) => {
            return Ok(vec![diag(
                "OOXML_XML_PARSE_ERROR",
                format!("failed to read {label} {}: {}", part.uri, err.message),
            )]);
        }
    };
    let root_diags = check_root_name(
        &part.uri,
        &info.root,
        expected_local,
        expected_ns,
        root_code,
    );
    if !root_diags.is_empty() {
        return Ok(root_diags);
    }

    let mut diagnostics = Vec::new();
    if let Some((code, order)) = order_check {
        diagnostics.extend(check_element_order(&part.uri, &info, order, code));
    }
    match part.content_type.as_str() {
        CONTENT_TYPE_XLSX_SHARED_STRINGS => {
            diagnostics.extend(check_shared_string_counts(&part.uri, &info));
        }
        CONTENT_TYPE_XLSX_STYLES => {
            diagnostics.extend(check_styles_counts(&part.uri, &info));
        }
        _ => {}
    }
    Ok(diagnostics)
}

fn read_xml_part_info(file: &str, part: &PartInfo) -> CliResult<XmlPartInfo> {
    let xml = zip_text(file, &part.entry_name)?;
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);
    let mut info = XmlPartInfo::default();
    let mut stack = Vec::<String>::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let element = xml_element_info(&e);
                if stack.is_empty() {
                    info.root = Some(element.clone());
                } else {
                    if stack.len() == 1 {
                        info.children.push(element.clone());
                    }
                    if let Some(parent) = stack.last() {
                        *info
                            .direct_child_counts
                            .entry((parent.clone(), element.local_name.clone()))
                            .or_insert(0) += 1;
                    }
                }
                stack.push(element.local_name);
            }
            Ok(Event::Empty(e)) => {
                let element = xml_element_info(&e);
                if stack.is_empty() {
                    info.root = Some(element);
                } else {
                    if stack.len() == 1 {
                        info.children.push(element.clone());
                    }
                    if let Some(parent) = stack.last() {
                        *info
                            .direct_child_counts
                            .entry((parent.clone(), element.local_name))
                            .or_insert(0) += 1;
                    }
                }
            }
            Ok(Event::End(_)) => {
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(crate::CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(info)
}

fn check_root_name(
    part_uri: &str,
    root: &Option<XmlElementInfo>,
    expected_local_name: &str,
    expected_namespace: &str,
    code: &str,
) -> Vec<Value> {
    let Some(root) = root else {
        return vec![diag(code, format!("{part_uri} has no XML root"))];
    };
    if root.local_name != expected_local_name || root.namespace != expected_namespace {
        return vec![diag(
            code,
            format!(
                "{part_uri} root is {{{}}}{}, expected {{{}}}{}",
                root.namespace, root.local_name, expected_namespace, expected_local_name
            ),
        )];
    }
    Vec::new()
}

fn check_element_order(
    part_uri: &str,
    info: &XmlPartInfo,
    order: fn(&str) -> usize,
    code: &str,
) -> Vec<Value> {
    if info.root.is_none() {
        return vec![diag(code, format!("{part_uri} has no XML root"))];
    }
    let mut diagnostics = Vec::new();
    let mut last_order = 0usize;
    let mut last_name = "";
    for child in &info.children {
        let current = order(&child.local_name);
        if current == 0 {
            continue;
        }
        if last_order > current {
            diagnostics.push(diag(
                code,
                format!(
                    "{part_uri} has <{}> after <{last_name}>; expected schema child order",
                    child.local_name
                ),
            ));
            continue;
        }
        last_order = current;
        last_name = &child.local_name;
    }
    diagnostics
}

fn check_shared_string_counts(part_uri: &str, info: &XmlPartInfo) -> Vec<Value> {
    let items = info
        .children
        .iter()
        .filter(|child| child.local_name == "si")
        .count();
    let Some(root) = &info.root else {
        return Vec::new();
    };
    let (count, count_present, count_ok) = optional_unsigned_int_attr(root, "count");
    let (unique_count, unique_present, unique_ok) = optional_unsigned_int_attr(root, "uniqueCount");

    let mut diagnostics = Vec::new();
    if count_present && !count_ok {
        diagnostics.push(diag(
            "XLSX_SHARED_STRINGS_COUNTS",
            format!(
                "{} <sst> count {:?} is not a valid unsigned integer",
                part_uri,
                root.attrs.get("count").cloned().unwrap_or_default()
            ),
        ));
    }
    if unique_present && !unique_ok {
        diagnostics.push(diag(
            "XLSX_SHARED_STRINGS_COUNTS",
            format!(
                "{} <sst> uniqueCount {:?} is not a valid unsigned integer",
                part_uri,
                root.attrs.get("uniqueCount").cloned().unwrap_or_default()
            ),
        ));
    }
    if count_present && !unique_present {
        diagnostics.push(diag(
            "XLSX_SHARED_STRINGS_COUNTS",
            format!("{part_uri} <sst> uses count without required uniqueCount"),
        ));
    }
    if unique_present && !count_present {
        diagnostics.push(diag(
            "XLSX_SHARED_STRINGS_COUNTS",
            format!("{part_uri} <sst> uses uniqueCount without required count"),
        ));
    }
    if unique_present && unique_ok && unique_count != items {
        diagnostics.push(diag(
            "XLSX_SHARED_STRINGS_COUNTS",
            format!(
                "{part_uri} <sst> uniqueCount is {unique_count} but contains {items} <si> entries"
            ),
        ));
    }
    let _ = count;
    diagnostics
}

fn check_styles_counts(part_uri: &str, info: &XmlPartInfo) -> Vec<Value> {
    let mut diagnostics = Vec::new();
    diagnostics.extend(check_style_collection_count(
        part_uri, info, "numFmts", "numFmt",
    ));
    diagnostics.extend(check_style_collection_count(
        part_uri, info, "cellXfs", "xf",
    ));
    diagnostics
}

fn check_style_collection_count(
    part_uri: &str,
    info: &XmlPartInfo,
    collection_name: &str,
    child_name: &str,
) -> Vec<Value> {
    let Some(collection) = info
        .children
        .iter()
        .find(|child| child.local_name == collection_name)
    else {
        return Vec::new();
    };
    let (declared, present, ok) = optional_unsigned_int_attr(collection, "count");
    if !present {
        return Vec::new();
    }
    if !ok {
        return vec![diag(
            "XLSX_STYLES_COUNT_MISMATCH",
            format!(
                "{} <{}> count {:?} is not a valid unsigned integer",
                part_uri,
                collection_name,
                collection.attrs.get("count").cloned().unwrap_or_default()
            ),
        )];
    }
    let actual = info
        .direct_child_counts
        .get(&(collection_name.to_string(), child_name.to_string()))
        .copied()
        .unwrap_or(0);
    if declared != actual {
        return vec![diag(
            "XLSX_STYLES_COUNT_MISMATCH",
            format!(
                "{part_uri} <{collection_name}> count is {declared} but contains {actual} <{child_name}> entries"
            ),
        )];
    }
    Vec::new()
}

fn optional_unsigned_int_attr(elem: &XmlElementInfo, name: &str) -> (usize, bool, bool) {
    let Some(raw) = elem.attrs.get(name) else {
        return (0, false, false);
    };
    let raw = raw.trim();
    if raw.is_empty() {
        return (0, true, false);
    }
    match raw.parse::<usize>() {
        Ok(value) => (value, true, true),
        Err(_) => (0, true, false),
    }
}

fn parse_relationship_part(
    file: &str,
    entry_name: &str,
) -> Result<Vec<RelationshipRecord>, String> {
    let xml = zip_text(file, entry_name).map_err(|err| err.message)?;
    relationship_records_from_xml(&xml)
}

fn relationship_records_from_xml(xml: &str) -> Result<Vec<RelationshipRecord>, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut rels = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "Relationship" =>
            {
                rels.push(RelationshipRecord {
                    id: attr_exact(&e, "Id").unwrap_or_default(),
                    rel_type: attr_exact(&e, "Type").unwrap_or_default(),
                    target: attr_exact(&e, "Target").unwrap_or_default(),
                    target_mode: attr_exact(&e, "TargetMode").unwrap_or_default(),
                });
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(err.to_string()),
            _ => {}
        }
    }
    Ok(rels)
}

fn expected_content_types_for_part(part_uri: &str) -> Vec<&'static str> {
    let uri = normalize_uri(part_uri);
    let base = base_name(&uri);
    match uri.as_str() {
        _ if is_rels_uri(&uri) => vec![CONTENT_TYPE_RELS],
        "/xl/workbook.xml" => xlsx_workbook_content_types(),
        "/xl/sharedStrings.xml" => vec![CONTENT_TYPE_XLSX_SHARED_STRINGS],
        "/xl/styles.xml" => vec![CONTENT_TYPE_XLSX_STYLES],
        "/xl/calcChain.xml" => vec![CONTENT_TYPE_XLSX_CALC_CHAIN],
        "/ppt/presentation.xml" => pptx_presentation_content_types(),
        "/ppt/tableStyles.xml" => vec![CONTENT_TYPE_PPTX_TABLE_STYLES],
        "/ppt/commentAuthors.xml" => vec![CONTENT_TYPE_PPTX_COMMENT_AUTHORS],
        "/ppt/presProps.xml" => vec![CONTENT_TYPE_PPTX_PRES_PROPS],
        "/ppt/viewProps.xml" => vec![CONTENT_TYPE_PPTX_VIEW_PROPS],
        "/word/document.xml" => docx_document_content_types(),
        "/word/styles.xml" => vec![CONTENT_TYPE_DOCX_STYLES],
        "/word/numbering.xml" => vec![CONTENT_TYPE_DOCX_NUMBERING],
        "/word/footnotes.xml" => vec![CONTENT_TYPE_DOCX_FOOTNOTES],
        "/word/endnotes.xml" => vec![CONTENT_TYPE_DOCX_ENDNOTES],
        "/word/comments.xml" => vec![CONTENT_TYPE_DOCX_COMMENTS],
        _ if uri.starts_with("/xl/worksheets/") && uri.ends_with(".xml") => {
            vec![CONTENT_TYPE_XLSX_WORKSHEET]
        }
        _ if uri.starts_with("/xl/chartsheets/") && uri.ends_with(".xml") => {
            vec![CONTENT_TYPE_XLSX_CHARTSHEET]
        }
        _ if uri.starts_with("/xl/dialogSheets/") && uri.ends_with(".xml") => {
            vec![CONTENT_TYPE_XLSX_DIALOGSHEET]
        }
        _ if uri.starts_with("/xl/tables/") && uri.ends_with(".xml") => {
            vec![CONTENT_TYPE_XLSX_TABLE]
        }
        _ if uri.starts_with("/xl/charts/") && uri.ends_with(".xml") => {
            vec![CONTENT_TYPE_CHART]
        }
        _ if uri.starts_with("/xl/drawings/") && uri.ends_with(".xml") => {
            vec![CONTENT_TYPE_DRAWING]
        }
        _ if uri.starts_with("/xl/pivotTables/") && uri.ends_with(".xml") => {
            vec![CONTENT_TYPE_XLSX_PIVOT_TABLE]
        }
        _ if uri.starts_with("/xl/pivotCache/")
            && base.starts_with("pivotCacheDefinition")
            && uri.ends_with(".xml") =>
        {
            vec![CONTENT_TYPE_XLSX_PIVOT_CACHE]
        }
        _ if uri.starts_with("/xl/pivotCache/")
            && base.starts_with("pivotCacheRecords")
            && uri.ends_with(".xml") =>
        {
            vec![CONTENT_TYPE_XLSX_PIVOT_RECORDS]
        }
        _ if uri.starts_with("/xl/comments") && uri.ends_with(".xml") => {
            vec![CONTENT_TYPE_XLSX_COMMENTS]
        }
        _ if uri.starts_with("/xl/drawings/") && uri.ends_with(".vml") => {
            vec![CONTENT_TYPE_XLSX_VML]
        }
        _ if uri.starts_with("/word/header") && uri.ends_with(".xml") => {
            vec![CONTENT_TYPE_DOCX_HEADER]
        }
        _ if uri.starts_with("/word/footer") && uri.ends_with(".xml") => {
            vec![CONTENT_TYPE_DOCX_FOOTER]
        }
        _ if uri.starts_with("/ppt/slides/") && uri.ends_with(".xml") => {
            vec![CONTENT_TYPE_PPTX_SLIDE]
        }
        _ if uri.starts_with("/ppt/slideLayouts/") && uri.ends_with(".xml") => {
            vec![CONTENT_TYPE_PPTX_SLIDE_LAYOUT]
        }
        _ if uri.starts_with("/ppt/slideMasters/") && uri.ends_with(".xml") => {
            vec![CONTENT_TYPE_PPTX_SLIDE_MASTER]
        }
        _ if uri.starts_with("/ppt/notesSlides/") && uri.ends_with(".xml") => {
            vec![CONTENT_TYPE_PPTX_NOTES_SLIDE]
        }
        _ if uri.starts_with("/ppt/notesMasters/") && uri.ends_with(".xml") => {
            vec![CONTENT_TYPE_PPTX_NOTES_MASTER]
        }
        _ if uri.starts_with("/ppt/theme/") && uri.ends_with(".xml") => {
            vec![CONTENT_TYPE_PPTX_THEME]
        }
        _ if uri.starts_with("/ppt/charts/") && uri.ends_with(".xml") => {
            vec![CONTENT_TYPE_CHART]
        }
        _ if uri.starts_with("/ppt/comments/") && uri.ends_with(".xml") => {
            vec![CONTENT_TYPE_PPTX_COMMENTS]
        }
        _ => Vec::new(),
    }
}

fn expected_relationship_target_content_types(
    source_uri: &str,
    target_uri: &str,
    rel_type: &str,
) -> Vec<&'static str> {
    let source_uri = normalize_uri(source_uri);
    let target_uri = normalize_uri(target_uri);
    match rel_type {
        REL_TYPE_OFFICE_DOCUMENT => {
            if target_uri.starts_with("/xl/") {
                xlsx_workbook_content_types()
            } else if target_uri.starts_with("/ppt/") {
                pptx_presentation_content_types()
            } else if target_uri.starts_with("/word/") {
                docx_document_content_types()
            } else {
                Vec::new()
            }
        }
        REL_TYPE_XLSX_WORKSHEET => vec![CONTENT_TYPE_XLSX_WORKSHEET],
        REL_TYPE_XLSX_CHARTSHEET => vec![CONTENT_TYPE_XLSX_CHARTSHEET],
        REL_TYPE_XLSX_DIALOGSHEET => vec![CONTENT_TYPE_XLSX_DIALOGSHEET],
        REL_TYPE_XLSX_SHARED_STRINGS => vec![CONTENT_TYPE_XLSX_SHARED_STRINGS],
        REL_TYPE_STYLES if source_uri.starts_with("/word/") || target_uri.starts_with("/word/") => {
            vec![CONTENT_TYPE_DOCX_STYLES]
        }
        REL_TYPE_STYLES if source_uri.starts_with("/xl/") || target_uri.starts_with("/xl/") => {
            vec![CONTENT_TYPE_XLSX_STYLES]
        }
        REL_TYPE_DOCX_NUMBERING => vec![CONTENT_TYPE_DOCX_NUMBERING],
        REL_TYPE_DOCX_HEADER => vec![CONTENT_TYPE_DOCX_HEADER],
        REL_TYPE_DOCX_FOOTER => vec![CONTENT_TYPE_DOCX_FOOTER],
        REL_TYPE_DOCX_FOOTNOTES => vec![CONTENT_TYPE_DOCX_FOOTNOTES],
        REL_TYPE_DOCX_ENDNOTES => vec![CONTENT_TYPE_DOCX_ENDNOTES],
        REL_TYPE_XLSX_CALC_CHAIN => vec![CONTENT_TYPE_XLSX_CALC_CHAIN],
        REL_TYPE_XLSX_TABLE => vec![CONTENT_TYPE_XLSX_TABLE],
        REL_TYPE_XLSX_DRAWING => vec![CONTENT_TYPE_DRAWING],
        REL_TYPE_CHART => vec![CONTENT_TYPE_CHART],
        REL_TYPE_XLSX_PIVOT_TABLE => vec![CONTENT_TYPE_XLSX_PIVOT_TABLE],
        REL_TYPE_XLSX_PIVOT_CACHE => vec![CONTENT_TYPE_XLSX_PIVOT_CACHE],
        REL_TYPE_XLSX_PIVOT_RECORDS => vec![CONTENT_TYPE_XLSX_PIVOT_RECORDS],
        REL_TYPE_COMMENTS if source_uri.starts_with("/ppt/") || target_uri.starts_with("/ppt/") => {
            vec![CONTENT_TYPE_PPTX_COMMENTS]
        }
        REL_TYPE_COMMENTS if source_uri.starts_with("/xl/") || target_uri.starts_with("/xl/") => {
            vec![CONTENT_TYPE_XLSX_COMMENTS]
        }
        REL_TYPE_COMMENTS
            if source_uri.starts_with("/word/") || target_uri.starts_with("/word/") =>
        {
            vec![CONTENT_TYPE_DOCX_COMMENTS]
        }
        REL_TYPE_PPTX_SLIDE => vec![CONTENT_TYPE_PPTX_SLIDE],
        REL_TYPE_PPTX_SLIDE_LAYOUT => vec![CONTENT_TYPE_PPTX_SLIDE_LAYOUT],
        REL_TYPE_PPTX_SLIDE_MASTER => vec![CONTENT_TYPE_PPTX_SLIDE_MASTER],
        REL_TYPE_PPTX_NOTES_SLIDE => vec![CONTENT_TYPE_PPTX_NOTES_SLIDE],
        REL_TYPE_PPTX_NOTES_MASTER => vec![CONTENT_TYPE_PPTX_NOTES_MASTER],
        REL_TYPE_OFFICE_THEME => vec![CONTENT_TYPE_PPTX_THEME],
        REL_TYPE_PPTX_COMMENT_AUTHORS => vec![CONTENT_TYPE_PPTX_COMMENT_AUTHORS],
        _ => Vec::new(),
    }
}

fn xlsx_workbook_content_types() -> Vec<&'static str> {
    vec![
        CONTENT_TYPE_XLSX_WORKBOOK,
        CONTENT_TYPE_XLSX_WORKBOOK_MACRO,
        CONTENT_TYPE_XLSX_WORKBOOK_TEMPLATE,
        CONTENT_TYPE_XLSX_WORKBOOK_ADDIN,
    ]
}

fn pptx_presentation_content_types() -> Vec<&'static str> {
    vec![
        CONTENT_TYPE_PPTX_PRESENTATION,
        CONTENT_TYPE_PPTX_PRESENTATION_MACRO,
        CONTENT_TYPE_PPTX_TEMPLATE,
        CONTENT_TYPE_PPTX_TEMPLATE_MACRO,
        CONTENT_TYPE_PPTX_SLIDESHOW,
        CONTENT_TYPE_PPTX_SLIDESHOW_MACRO,
    ]
}

fn docx_document_content_types() -> Vec<&'static str> {
    vec![
        CONTENT_TYPE_DOCX_DOCUMENT,
        CONTENT_TYPE_DOCX_DOCUMENT_MACRO,
        CONTENT_TYPE_DOCX_TEMPLATE,
        CONTENT_TYPE_DOCX_TEMPLATE_MACRO,
    ]
}

fn is_xlsx_workbook_content_type(content_type: &str) -> bool {
    matches!(
        content_type,
        CONTENT_TYPE_XLSX_WORKBOOK
            | CONTENT_TYPE_XLSX_WORKBOOK_MACRO
            | CONTENT_TYPE_XLSX_WORKBOOK_TEMPLATE
            | CONTENT_TYPE_XLSX_WORKBOOK_ADDIN
    )
}

fn is_pptx_presentation_content_type(content_type: &str) -> bool {
    matches!(
        content_type,
        CONTENT_TYPE_PPTX_PRESENTATION
            | CONTENT_TYPE_PPTX_PRESENTATION_MACRO
            | CONTENT_TYPE_PPTX_TEMPLATE
            | CONTENT_TYPE_PPTX_TEMPLATE_MACRO
            | CONTENT_TYPE_PPTX_SLIDESHOW
            | CONTENT_TYPE_PPTX_SLIDESHOW_MACRO
    )
}

fn rels_part_for_source(source_uri: &str) -> String {
    if normalize_uri(source_uri) == "/" {
        "_rels/.rels".to_string()
    } else {
        relationships_part_for(source_uri.trim_start_matches('/'))
    }
}

fn relationship_label(source_uri: &str, rel: &RelationshipRecord) -> String {
    if rel.id.is_empty() {
        format!("{source_uri} relationship")
    } else {
        format!("{source_uri} relationship {}", rel.id)
    }
}

fn looks_external_relationship_target(target: &str) -> bool {
    let lowered = target.trim().to_ascii_lowercase();
    lowered.contains("://")
        || lowered.starts_with("mailto:")
        || lowered.starts_with("file:")
        || lowered.starts_with("urn:")
}

fn xml_element_info(e: &BytesStart<'_>) -> XmlElementInfo {
    XmlElementInfo {
        local_name: local_name(e.name().as_ref()).to_string(),
        namespace: element_namespace(e),
        attrs: decode_local_xml_attrs(e),
    }
}

fn element_namespace(e: &BytesStart<'_>) -> String {
    let name = e.name();
    let raw = std::str::from_utf8(name.as_ref()).unwrap_or_default();
    let prefix = raw.rsplit_once(':').map(|(prefix, _)| prefix);
    let wanted = match prefix {
        Some(prefix) => format!("xmlns:{prefix}"),
        None => "xmlns".to_string(),
    };
    e.attributes()
        .with_checks(false)
        .flatten()
        .find_map(|attr| {
            if String::from_utf8_lossy(attr.key.as_ref()) == wanted {
                Some(decode_xml_text(attr.value.as_ref()))
            } else {
                None
            }
        })
        .unwrap_or_default()
}

fn is_rels_uri(uri: &str) -> bool {
    let normalized = normalize_uri(uri);
    normalized == "/_rels/.rels"
        || (normalized.ends_with(".rels") && normalized.contains("/_rels/"))
}

fn normalize_uri(uri: &str) -> String {
    let mut parts = Vec::new();
    let normalized = uri.replace('\\', "/");
    for part in normalized.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    format!("/{}", parts.join("/"))
}

fn file_extension(uri: &str) -> &str {
    base_name(uri)
        .rsplit_once('.')
        .map(|(_, extension)| extension)
        .unwrap_or_default()
}

fn base_name(uri: &str) -> &str {
    uri.rsplit('/').next().unwrap_or(uri)
}

fn worksheet_child_order(name: &str) -> usize {
    order_index(
        name,
        &[
            "sheetPr",
            "dimension",
            "sheetViews",
            "sheetFormatPr",
            "cols",
            "sheetData",
            "sheetCalcPr",
            "sheetProtection",
            "protectedRanges",
            "scenarios",
            "autoFilter",
            "sortState",
            "dataConsolidate",
            "customSheetViews",
            "mergeCells",
            "phoneticPr",
            "conditionalFormatting",
            "dataValidations",
            "hyperlinks",
            "printOptions",
            "pageMargins",
            "pageSetup",
            "headerFooter",
            "rowBreaks",
            "colBreaks",
            "customProperties",
            "cellWatches",
            "ignoredErrors",
            "smartTags",
            "drawing",
            "legacyDrawing",
            "legacyDrawingHF",
            "drawingHF",
            "picture",
            "oleObjects",
            "controls",
            "webPublishItems",
            "tableParts",
            "extLst",
        ],
    )
}

fn slide_child_order(name: &str) -> usize {
    order_index(
        name,
        &["cSld", "clrMapOvr", "transition", "timing", "extLst"],
    )
}

fn slide_layout_child_order(name: &str) -> usize {
    order_index(
        name,
        &["cSld", "clrMapOvr", "transition", "timing", "hf", "extLst"],
    )
}

fn slide_master_child_order(name: &str) -> usize {
    order_index(
        name,
        &[
            "cSld",
            "clrMap",
            "sldLayoutIdLst",
            "transition",
            "timing",
            "hf",
            "txStyles",
            "extLst",
        ],
    )
}

fn order_index(name: &str, ordered_names: &[&str]) -> usize {
    ordered_names
        .iter()
        .position(|candidate| *candidate == name)
        .map(|idx| idx + 1)
        .unwrap_or(0)
}

fn diag(code: &str, message: impl Into<String>) -> Value {
    json!({
        "code": code,
        "severity": "error",
        "message": message.into(),
    })
}
