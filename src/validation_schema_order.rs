use quick_xml::NsReader;
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::{Namespace, ResolveResult};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::{
    CliResult, InspectPackageKind, attr, find_docx_document_part, find_xlsx_workbook_part,
    is_xlsx_chart_part, is_xlsx_pivot_table_part, is_xlsx_table_part, is_xlsx_worksheet_part,
    local_name, zip_text,
};

const SPREADSHEET_NS: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const SPREADSHEET_STRICT_NS: &str = "http://purl.oclc.org/ooxml/spreadsheetml/main";
const PRESENTATION_NS: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
const PRESENTATION_STRICT_NS: &str = "http://purl.oclc.org/ooxml/presentationml/main";
const WORDPROCESSING_NS: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const WORDPROCESSING_STRICT_NS: &str = "http://purl.oclc.org/ooxml/wordprocessingml/main";
const DRAWING_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const DRAWING_STRICT_NS: &str = "http://purl.oclc.org/ooxml/drawingml/main";
const CHART_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/chart";
const CHART_STRICT_NS: &str = "http://purl.oclc.org/ooxml/drawingml/chart";
const MC_NS: &str = "http://schemas.openxmlformats.org/markup-compatibility/2006";

const CONTENT_TYPE_CHART: &str =
    "application/vnd.openxmlformats-officedocument.drawingml.chart+xml";
const CONTENT_TYPE_PPTX_SLIDE: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.slide+xml";
const CONTENT_TYPE_PPTX_SLIDE_LAYOUT: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml";
const CONTENT_TYPE_PPTX_SLIDE_MASTER: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NamespaceFamily {
    Spreadsheet,
    Presentation,
    Wordprocessing,
    Drawing,
    Chart,
}

#[derive(Clone, Copy)]
enum RuleNamespace {
    Parent,
    Drawing,
}

#[derive(Clone, Copy)]
struct ChildRule {
    namespace: RuleNamespace,
    local_name: &'static str,
    position: Option<u16>,
    severity: &'static str,
}

#[derive(Clone, Copy)]
struct SequenceTable {
    label: &'static str,
    parent_namespace: NamespaceFamily,
    parent_local_name: &'static str,
    children: &'static [ChildRule],
}

const fn child(local_name: &'static str, position: u16) -> ChildRule {
    ChildRule {
        namespace: RuleNamespace::Parent,
        local_name,
        position: Some(position),
        severity: "error",
    }
}

const fn drawing_child(local_name: &'static str, position: u16) -> ChildRule {
    ChildRule {
        namespace: RuleNamespace::Drawing,
        local_name,
        position: Some(position),
        severity: "error",
    }
}

const fn unordered_child(local_name: &'static str) -> ChildRule {
    ChildRule {
        namespace: RuleNamespace::Parent,
        local_name,
        position: None,
        severity: "error",
    }
}

// These tables mirror the top-level sequence particles in ISO/IEC 29500.
// Optional/repeating cardinality is deliberately out of scope: this validator
// catches unknown and misplaced children while the SDK remains the XSD oracle.
const WORKSHEET_CHILDREN: &[ChildRule] = &[
    child("sheetPr", 1),
    child("dimension", 2),
    child("sheetViews", 3),
    child("sheetFormatPr", 4),
    child("cols", 5),
    child("sheetData", 6),
    child("sheetCalcPr", 7),
    child("sheetProtection", 8),
    child("protectedRanges", 9),
    child("scenarios", 10),
    child("autoFilter", 11),
    child("sortState", 12),
    child("dataConsolidate", 13),
    child("customSheetViews", 14),
    child("mergeCells", 15),
    child("phoneticPr", 16),
    child("conditionalFormatting", 17),
    child("dataValidations", 18),
    child("hyperlinks", 19),
    child("printOptions", 20),
    child("pageMargins", 21),
    child("pageSetup", 22),
    child("headerFooter", 23),
    child("rowBreaks", 24),
    child("colBreaks", 25),
    child("customProperties", 26),
    child("cellWatches", 27),
    child("ignoredErrors", 28),
    child("smartTags", 29),
    child("drawing", 30),
    child("legacyDrawing", 31),
    child("legacyDrawingHF", 32),
    child("drawingHF", 33),
    child("picture", 34),
    child("oleObjects", 35),
    child("controls", 36),
    child("webPublishItems", 37),
    child("tableParts", 38),
    child("extLst", 39),
];

const WORKBOOK_CHILDREN: &[ChildRule] = &[
    child("fileVersion", 1),
    child("fileSharing", 2),
    child("workbookPr", 3),
    child("workbookProtection", 4),
    child("bookViews", 5),
    child("sheets", 6),
    child("functionGroups", 7),
    child("externalReferences", 8),
    child("definedNames", 9),
    child("calcPr", 10),
    child("oleSize", 11),
    child("customWorkbookViews", 12),
    child("pivotCaches", 13),
    child("smartTagPr", 14),
    child("smartTagTypes", 15),
    child("webPublishing", 16),
    child("fileRecoveryPr", 17),
    child("webPublishObjects", 18),
    child("extLst", 19),
];

const TABLE_CHILDREN: &[ChildRule] = &[
    child("autoFilter", 1),
    child("sortState", 2),
    child("tableColumns", 3),
    child("tableStyleInfo", 4),
    child("extLst", 5),
];

const PIVOT_TABLE_CHILDREN: &[ChildRule] = &[
    child("location", 1),
    child("pivotFields", 2),
    child("rowFields", 3),
    child("rowItems", 4),
    child("colFields", 5),
    child("colItems", 6),
    child("pageFields", 7),
    child("dataFields", 8),
    child("formats", 9),
    child("conditionalFormats", 10),
    child("chartFormats", 11),
    child("pivotHierarchies", 12),
    child("pivotTableStyleInfo", 13),
    child("filters", 14),
    child("rowHierarchiesUsage", 15),
    child("colHierarchiesUsage", 16),
    child("extLst", 17),
];

const SLIDE_CHILDREN: &[ChildRule] = &[
    child("cSld", 1),
    child("clrMapOvr", 2),
    child("transition", 3),
    child("timing", 4),
    child("extLst", 5),
];

const SLIDE_LAYOUT_CHILDREN: &[ChildRule] = &[
    child("cSld", 1),
    child("clrMapOvr", 2),
    child("transition", 3),
    child("timing", 4),
    child("hf", 5),
    child("extLst", 6),
];

const SLIDE_MASTER_CHILDREN: &[ChildRule] = &[
    child("cSld", 1),
    child("clrMap", 2),
    child("sldLayoutIdLst", 3),
    child("transition", 4),
    child("timing", 5),
    child("hf", 6),
    child("txStyles", 7),
    child("extLst", 8),
];

const COMMON_SLIDE_DATA_CHILDREN: &[ChildRule] = &[
    child("bg", 1),
    child("spTree", 2),
    child("custDataLst", 3),
    child("controls", 4),
    child("extLst", 5),
];

const SHAPE_CHILDREN: &[ChildRule] = &[
    child("nvSpPr", 1),
    child("spPr", 2),
    child("style", 3),
    child("txBody", 4),
    child("extLst", 5),
];

const GRAPHIC_FRAME_CHILDREN: &[ChildRule] = &[
    child("nvGraphicFramePr", 1),
    child("xfrm", 2),
    drawing_child("graphic", 3),
    child("extLst", 4),
];

const DOCUMENT_CHILDREN: &[ChildRule] = &[child("background", 1), child("body", 2)];

const BODY_CHILDREN: &[ChildRule] = &[
    child("altChunk", 1),
    child("customXml", 1),
    child("sdt", 1),
    child("p", 1),
    child("tbl", 1),
    child("proofErr", 1),
    child("permStart", 1),
    child("permEnd", 1),
    child("bookmarkStart", 1),
    child("bookmarkEnd", 1),
    child("commentRangeStart", 1),
    child("commentRangeEnd", 1),
    child("moveFromRangeStart", 1),
    child("moveFromRangeEnd", 1),
    child("moveToRangeStart", 1),
    child("moveToRangeEnd", 1),
    child("customXmlInsRangeStart", 1),
    child("customXmlInsRangeEnd", 1),
    child("customXmlDelRangeStart", 1),
    child("customXmlDelRangeEnd", 1),
    child("customXmlMoveFromRangeStart", 1),
    child("customXmlMoveFromRangeEnd", 1),
    child("customXmlMoveToRangeStart", 1),
    child("customXmlMoveToRangeEnd", 1),
    child("ins", 1),
    child("del", 1),
    child("moveFrom", 1),
    child("moveTo", 1),
    child("contentPart", 1),
    child("sectPr", 2),
];

const PARAGRAPH_CHILDREN: &[ChildRule] = &[
    child("pPr", 1),
    child("customXml", 2),
    child("fldSimple", 2),
    child("hyperlink", 2),
    child("sdt", 2),
    child("proofErr", 2),
    child("permStart", 2),
    child("permEnd", 2),
    child("bookmarkStart", 2),
    child("bookmarkEnd", 2),
    child("commentRangeStart", 2),
    child("commentRangeEnd", 2),
    child("moveFromRangeStart", 2),
    child("moveFromRangeEnd", 2),
    child("moveToRangeStart", 2),
    child("moveToRangeEnd", 2),
    child("customXmlInsRangeStart", 2),
    child("customXmlInsRangeEnd", 2),
    child("customXmlDelRangeStart", 2),
    child("customXmlDelRangeEnd", 2),
    child("customXmlMoveFromRangeStart", 2),
    child("customXmlMoveFromRangeEnd", 2),
    child("customXmlMoveToRangeStart", 2),
    child("customXmlMoveToRangeEnd", 2),
    child("ins", 2),
    child("del", 2),
    child("moveFrom", 2),
    child("moveTo", 2),
    child("contentPart", 2),
    child("r", 2),
    child("bdo", 2),
    child("dir", 2),
    child("subDoc", 2),
];

const RUN_CHILDREN: &[ChildRule] = &[
    child("rPr", 1),
    child("br", 2),
    child("t", 2),
    child("delText", 2),
    child("instrText", 2),
    child("delInstrText", 2),
    child("noBreakHyphen", 2),
    child("softHyphen", 2),
    child("dayShort", 2),
    child("monthShort", 2),
    child("yearShort", 2),
    child("dayLong", 2),
    child("monthLong", 2),
    child("yearLong", 2),
    child("annotationRef", 2),
    child("footnoteRef", 2),
    child("endnoteRef", 2),
    child("separator", 2),
    child("continuationSeparator", 2),
    child("sym", 2),
    child("pgNum", 2),
    child("cr", 2),
    child("tab", 2),
    child("object", 2),
    child("pict", 2),
    child("fldChar", 2),
    child("ruby", 2),
    child("footnoteReference", 2),
    child("endnoteReference", 2),
    child("commentReference", 2),
    child("drawing", 2),
    child("ptab", 2),
    child("lastRenderedPageBreak", 2),
];

const WORD_TABLE_CHILDREN: &[ChildRule] = &[
    unordered_child("bookmarkStart"),
    unordered_child("bookmarkEnd"),
    unordered_child("commentRangeStart"),
    unordered_child("commentRangeEnd"),
    unordered_child("moveFromRangeStart"),
    unordered_child("moveFromRangeEnd"),
    unordered_child("moveToRangeStart"),
    unordered_child("moveToRangeEnd"),
    unordered_child("customXmlInsRangeStart"),
    unordered_child("customXmlInsRangeEnd"),
    unordered_child("customXmlDelRangeStart"),
    unordered_child("customXmlDelRangeEnd"),
    unordered_child("customXmlMoveFromRangeStart"),
    unordered_child("customXmlMoveFromRangeEnd"),
    unordered_child("customXmlMoveToRangeStart"),
    unordered_child("customXmlMoveToRangeEnd"),
    child("tblPr", 1),
    child("tblGrid", 2),
    child("tr", 3),
    child("customXml", 3),
    child("sdt", 3),
    child("proofErr", 3),
    child("permStart", 3),
    child("permEnd", 3),
    child("ins", 3),
    child("del", 3),
    child("moveFrom", 3),
    child("moveTo", 3),
    child("contentPart", 3),
];

const CHART_SPACE_CHILDREN: &[ChildRule] = &[
    child("date1904", 1),
    child("lang", 2),
    child("roundedCorners", 3),
    child("style", 4),
    child("clrMapOvr", 5),
    child("pivotSource", 6),
    child("protection", 7),
    child("chart", 8),
    child("spPr", 9),
    child("txPr", 10),
    child("externalData", 11),
    child("printSettings", 12),
    child("userShapes", 13),
    child("extLst", 14),
];

const SEQUENCE_TABLES: &[SequenceTable] = &[
    SequenceTable {
        label: "CT_Worksheet",
        parent_namespace: NamespaceFamily::Spreadsheet,
        parent_local_name: "worksheet",
        children: WORKSHEET_CHILDREN,
    },
    SequenceTable {
        label: "CT_Workbook",
        parent_namespace: NamespaceFamily::Spreadsheet,
        parent_local_name: "workbook",
        children: WORKBOOK_CHILDREN,
    },
    SequenceTable {
        label: "CT_Table",
        parent_namespace: NamespaceFamily::Spreadsheet,
        parent_local_name: "table",
        children: TABLE_CHILDREN,
    },
    SequenceTable {
        label: "CT_PivotTableDefinition",
        parent_namespace: NamespaceFamily::Spreadsheet,
        parent_local_name: "pivotTableDefinition",
        children: PIVOT_TABLE_CHILDREN,
    },
    SequenceTable {
        label: "CT_Slide",
        parent_namespace: NamespaceFamily::Presentation,
        parent_local_name: "sld",
        children: SLIDE_CHILDREN,
    },
    SequenceTable {
        label: "CT_SlideLayout",
        parent_namespace: NamespaceFamily::Presentation,
        parent_local_name: "sldLayout",
        children: SLIDE_LAYOUT_CHILDREN,
    },
    SequenceTable {
        label: "CT_SlideMaster",
        parent_namespace: NamespaceFamily::Presentation,
        parent_local_name: "sldMaster",
        children: SLIDE_MASTER_CHILDREN,
    },
    SequenceTable {
        label: "CT_CommonSlideData",
        parent_namespace: NamespaceFamily::Presentation,
        parent_local_name: "cSld",
        children: COMMON_SLIDE_DATA_CHILDREN,
    },
    SequenceTable {
        label: "CT_Shape",
        parent_namespace: NamespaceFamily::Presentation,
        parent_local_name: "sp",
        children: SHAPE_CHILDREN,
    },
    SequenceTable {
        label: "CT_GraphicalObjectFrame",
        parent_namespace: NamespaceFamily::Presentation,
        parent_local_name: "graphicFrame",
        children: GRAPHIC_FRAME_CHILDREN,
    },
    SequenceTable {
        label: "CT_Document",
        parent_namespace: NamespaceFamily::Wordprocessing,
        parent_local_name: "document",
        children: DOCUMENT_CHILDREN,
    },
    SequenceTable {
        label: "CT_Body",
        parent_namespace: NamespaceFamily::Wordprocessing,
        parent_local_name: "body",
        children: BODY_CHILDREN,
    },
    SequenceTable {
        label: "CT_P",
        parent_namespace: NamespaceFamily::Wordprocessing,
        parent_local_name: "p",
        children: PARAGRAPH_CHILDREN,
    },
    SequenceTable {
        label: "CT_R",
        parent_namespace: NamespaceFamily::Wordprocessing,
        parent_local_name: "r",
        children: RUN_CHILDREN,
    },
    SequenceTable {
        label: "CT_Tbl",
        parent_namespace: NamespaceFamily::Wordprocessing,
        parent_local_name: "tbl",
        children: WORD_TABLE_CHILDREN,
    },
    SequenceTable {
        label: "CT_ChartSpace",
        parent_namespace: NamespaceFamily::Chart,
        parent_local_name: "chartSpace",
        children: CHART_SPACE_CHILDREN,
    },
];

#[derive(Default)]
struct ContentTypes {
    defaults: BTreeMap<String, String>,
    overrides: BTreeMap<String, String>,
}

impl ContentTypes {
    fn for_entry(&self, entry: &str) -> &str {
        let uri = format!("/{}", entry.trim_start_matches('/'));
        if let Some(content_type) = self.overrides.get(&uri) {
            return content_type;
        }
        let extension = Path::new(entry)
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        self.defaults
            .get(&extension)
            .map(String::as_str)
            .unwrap_or_default()
    }
}

#[derive(Clone, Copy)]
enum CoveredPartKind {
    Spreadsheet,
    Presentation,
    Wordprocessing,
    Chart,
}

pub(crate) fn validate_package_schema_order(
    file: &str,
    package_kind: InspectPackageKind,
    entries: &[String],
) -> CliResult<Vec<Value>> {
    let content_types = parse_content_types(file).unwrap_or_default();
    let entry_set = entries.iter().map(String::as_str).collect::<BTreeSet<_>>();
    let mut parts = BTreeMap::<String, CoveredPartKind>::new();

    match package_kind {
        InspectPackageKind::Xlsx => {
            if let Ok(workbook) = find_xlsx_workbook_part(file, entries)
                && entry_set.contains(workbook.as_str())
            {
                parts.insert(workbook, CoveredPartKind::Spreadsheet);
            }
            for entry in entries.iter().filter(|entry| entry.ends_with(".xml")) {
                let uri = format!("/{}", entry.trim_start_matches('/'));
                let content_type = content_types.for_entry(entry);
                if is_xlsx_worksheet_part(&uri, content_type)
                    || is_xlsx_table_part(&uri, content_type)
                    || is_xlsx_pivot_table_part(&uri, content_type)
                {
                    parts.insert(entry.clone(), CoveredPartKind::Spreadsheet);
                }
            }
        }
        InspectPackageKind::Pptx => {
            for entry in entries.iter().filter(|entry| entry.ends_with(".xml")) {
                let content_type = content_types.for_entry(entry);
                if matches!(
                    content_type,
                    CONTENT_TYPE_PPTX_SLIDE
                        | CONTENT_TYPE_PPTX_SLIDE_LAYOUT
                        | CONTENT_TYPE_PPTX_SLIDE_MASTER
                ) || is_presentation_part_path(entry)
                {
                    parts.insert(entry.clone(), CoveredPartKind::Presentation);
                }
            }
        }
        InspectPackageKind::Docx => {
            if let Ok(document) = find_docx_document_part(file, entries)
                && entry_set.contains(document.as_str())
            {
                parts.insert(document, CoveredPartKind::Wordprocessing);
            }
        }
        InspectPackageKind::Unknown => {}
    }

    for entry in entries.iter().filter(|entry| entry.ends_with(".xml")) {
        let content_type = content_types.for_entry(entry);
        let uri = format!("/{}", entry.trim_start_matches('/'));
        if content_type == CONTENT_TYPE_CHART
            || matches!(package_kind, InspectPackageKind::Xlsx)
                && is_xlsx_chart_part(&uri, content_type)
            || is_chart_part_path(entry)
        {
            parts.insert(entry.clone(), CoveredPartKind::Chart);
        }
    }

    let mut diagnostics = Vec::new();
    for (entry, part_kind) in parts {
        let part_uri = format!("/{}", entry.trim_start_matches('/'));
        let xml = zip_text(file, &entry)?;
        diagnostics.extend(validate_xml_part(&part_uri, &xml, part_kind));
    }
    diagnostics.sort_by_key(diagnostic_sort_key);
    Ok(diagnostics)
}

pub(crate) fn xlsx_workbook_schema_child_order(local: &str) -> i32 {
    WORKBOOK_CHILDREN
        .iter()
        .find(|rule| rule.local_name == local)
        .and_then(|rule| rule.position)
        .map_or(10_000, |position| i32::from(position) * 10)
}

fn parse_content_types(file: &str) -> CliResult<ContentTypes> {
    let xml = zip_text(file, "[Content_Types].xml")?;
    let mut reader = quick_xml::Reader::from_str(&xml);
    reader.config_mut().trim_text(true);
    let mut content_types = ContentTypes::default();
    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) | Ok(Event::Empty(element))
                if local_name(element.name().as_ref()) == "Default" =>
            {
                if let (Some(extension), Some(content_type)) =
                    (attr(&element, "Extension"), attr(&element, "ContentType"))
                {
                    content_types
                        .defaults
                        .insert(extension.to_ascii_lowercase(), content_type);
                }
            }
            Ok(Event::Start(element)) | Ok(Event::Empty(element))
                if local_name(element.name().as_ref()) == "Override" =>
            {
                if let (Some(part_name), Some(content_type)) =
                    (attr(&element, "PartName"), attr(&element, "ContentType"))
                {
                    content_types.overrides.insert(part_name, content_type);
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    Ok(content_types)
}

fn is_presentation_part_path(entry: &str) -> bool {
    let entry = entry.trim_start_matches('/');
    (entry.starts_with("ppt/slides/slide")
        || entry.starts_with("ppt/slideLayouts/slideLayout")
        || entry.starts_with("ppt/slideMasters/slideMaster"))
        && entry.ends_with(".xml")
}

fn is_chart_part_path(entry: &str) -> bool {
    let entry = entry.trim_start_matches('/');
    let in_chart_directory = entry.starts_with("xl/charts/")
        || entry.starts_with("ppt/charts/")
        || entry.starts_with("word/charts/");
    let numbered_chart_name = entry
        .rsplit('/')
        .next()
        .and_then(|name| name.strip_prefix("chart"))
        .and_then(|name| name.strip_suffix(".xml"))
        .is_some_and(|number| {
            !number.is_empty() && number.bytes().all(|byte| byte.is_ascii_digit())
        });
    in_chart_directory && numbered_chart_name
}

struct Frame {
    namespace_family: Option<NamespaceFamily>,
    xpath: String,
    table: Option<&'static SequenceTable>,
    last_position: u16,
    last_name: String,
    child_position: usize,
    child_counts: BTreeMap<(String, String), usize>,
    ignored: bool,
}

fn validate_xml_part(part_uri: &str, xml: &str, part_kind: CoveredPartKind) -> Vec<Value> {
    let mut reader = NsReader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut stack = Vec::<Frame>::new();
    let mut diagnostics = Vec::new();

    loop {
        let (resolved, event) = match reader.read_resolved_event() {
            Ok(value) => value,
            Err(error) => {
                diagnostics.push(json!({
                    "code": "XML_SCHEMA_ORDER_PARSE",
                    "severity": "error",
                    "message": format!("{part_uri} could not be parsed for schema child order: {error}"),
                    "part": part_uri,
                    "xpath": stack.last().map(|frame| frame.xpath.as_str()).unwrap_or("/"),
                }));
                break;
            }
        };
        match event {
            Event::Start(element) => {
                let namespace = resolved_namespace(resolved);
                push_element(
                    part_uri,
                    &element,
                    namespace,
                    part_kind,
                    &mut stack,
                    &mut diagnostics,
                );
            }
            Event::Empty(element) => {
                let namespace = resolved_namespace(resolved);
                push_element(
                    part_uri,
                    &element,
                    namespace,
                    part_kind,
                    &mut stack,
                    &mut diagnostics,
                );
                stack.pop();
            }
            Event::End(_) => {
                stack.pop();
            }
            Event::Eof => break,
            _ => {}
        }
    }
    diagnostics
}

fn push_element(
    part_uri: &str,
    element: &BytesStart<'_>,
    namespace: String,
    part_kind: CoveredPartKind,
    stack: &mut Vec<Frame>,
    diagnostics: &mut Vec<Value>,
) {
    let local = local_name(element.name().as_ref()).to_string();
    let namespace_family = namespace_family(&namespace);
    let ignored_by_parent = stack.last().is_some_and(|parent| parent.ignored);
    let ignored_wrapper = local == "AlternateContent" && namespace == MC_NS
        || local == "extLst"
            && stack.last().is_some_and(|parent| {
                parent.namespace_family.is_some() && parent.namespace_family == namespace_family
            });

    let (xpath, ignored) = if let Some(parent) = stack.last_mut() {
        parent.child_position += 1;
        let count = parent
            .child_counts
            .entry((namespace.clone(), local.clone()))
            .or_insert(0);
        *count += 1;
        let xpath = format!(
            "{}/{}[{}]",
            parent.xpath,
            canonical_qname(&namespace, &local),
            *count
        );
        let rule = parent
            .table
            .and_then(|table| find_child_rule(table, &namespace, &local));
        let foreign = parent.namespace_family != namespace_family && rule.is_none();
        let ignored = ignored_by_parent || ignored_wrapper || foreign;
        if !ignored {
            if let Some(table) = parent.table {
                match rule {
                    Some(rule) => {
                        if let Some(position) = rule.position {
                            if position < parent.last_position {
                                diagnostics.push(json!({
                                    "code": "XML_CHILD_ORDER",
                                    "severity": rule.severity,
                                    "message": format!(
                                        "{part_uri} {table_label} child <{local}> at child position {actual} appears after <{previous}>; expected schema sequence position {expected}",
                                        table_label = table.label,
                                        actual = parent.child_position,
                                        previous = parent.last_name,
                                        expected = position,
                                    ),
                                    "part": part_uri,
                                    "xpath": xpath,
                                    "element": local,
                                    "position": parent.child_position,
                                    "expectedPosition": position,
                                }));
                            } else {
                                parent.last_position = position;
                                parent.last_name.clone_from(&local);
                            }
                        }
                    }
                    None => {
                        let expected = parent.last_position.saturating_add(1).max(1);
                        diagnostics.push(json!({
                            "code": "XML_UNKNOWN_CHILD",
                            "severity": "error",
                            "message": format!(
                                "{part_uri} {table_label} has unknown child <{local}> at child position {actual}; expected a schema child at or after sequence position {expected}",
                                table_label = table.label,
                                actual = parent.child_position,
                            ),
                            "part": part_uri,
                            "xpath": xpath,
                            "element": local,
                            "position": parent.child_position,
                            "expectedPosition": expected,
                        }));
                    }
                }
            }
        }
        (xpath, ignored)
    } else {
        (
            format!("/{}[1]", canonical_qname(&namespace, &local)),
            ignored_wrapper,
        )
    };

    let table = if ignored || !part_kind_allows(part_kind, namespace_family) {
        None
    } else {
        sequence_table(namespace_family, &local)
    };
    stack.push(Frame {
        namespace_family,
        xpath,
        table,
        last_position: 0,
        last_name: String::new(),
        child_position: 0,
        child_counts: BTreeMap::new(),
        ignored,
    });
}

fn find_child_rule(
    table: &SequenceTable,
    namespace: &str,
    local: &str,
) -> Option<&'static ChildRule> {
    table.children.iter().find(|rule| {
        rule.local_name == local
            && match rule.namespace {
                RuleNamespace::Parent => namespace_is_family(namespace, table.parent_namespace),
                RuleNamespace::Drawing => namespace_is_family(namespace, NamespaceFamily::Drawing),
            }
    })
}

fn sequence_table(
    namespace: Option<NamespaceFamily>,
    local: &str,
) -> Option<&'static SequenceTable> {
    SEQUENCE_TABLES
        .iter()
        .find(|table| Some(table.parent_namespace) == namespace && table.parent_local_name == local)
}

fn part_kind_allows(kind: CoveredPartKind, namespace: Option<NamespaceFamily>) -> bool {
    matches!(
        (kind, namespace),
        (
            CoveredPartKind::Spreadsheet,
            Some(NamespaceFamily::Spreadsheet)
        ) | (
            CoveredPartKind::Presentation,
            Some(NamespaceFamily::Presentation)
        ) | (
            CoveredPartKind::Wordprocessing,
            Some(NamespaceFamily::Wordprocessing)
        ) | (CoveredPartKind::Chart, Some(NamespaceFamily::Chart))
    )
}

fn resolved_namespace(resolved: ResolveResult<'_>) -> String {
    match resolved {
        ResolveResult::Bound(Namespace(namespace)) => {
            String::from_utf8_lossy(namespace).into_owned()
        }
        ResolveResult::Unbound | ResolveResult::Unknown(_) => String::new(),
    }
}

fn namespace_family(namespace: &str) -> Option<NamespaceFamily> {
    match namespace {
        SPREADSHEET_NS | SPREADSHEET_STRICT_NS => Some(NamespaceFamily::Spreadsheet),
        PRESENTATION_NS | PRESENTATION_STRICT_NS => Some(NamespaceFamily::Presentation),
        WORDPROCESSING_NS | WORDPROCESSING_STRICT_NS => Some(NamespaceFamily::Wordprocessing),
        DRAWING_NS | DRAWING_STRICT_NS => Some(NamespaceFamily::Drawing),
        CHART_NS | CHART_STRICT_NS => Some(NamespaceFamily::Chart),
        _ => None,
    }
}

fn namespace_is_family(namespace: &str, family: NamespaceFamily) -> bool {
    namespace_family(namespace) == Some(family)
}

fn canonical_qname(namespace: &str, local: &str) -> String {
    let prefix = match namespace_family(namespace) {
        Some(NamespaceFamily::Spreadsheet) => "x",
        Some(NamespaceFamily::Presentation) => "p",
        Some(NamespaceFamily::Wordprocessing) => "w",
        Some(NamespaceFamily::Drawing) => "a",
        Some(NamespaceFamily::Chart) => "c",
        None if namespace == MC_NS => "mc",
        None => "ns",
    };
    format!("{prefix}:{local}")
}

fn diagnostic_sort_key(diagnostic: &Value) -> (String, String, String, String) {
    (
        diagnostic
            .get("part")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        diagnostic
            .get("xpath")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        diagnostic
            .get("code")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        diagnostic
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn namespace_uri(family: NamespaceFamily) -> &'static str {
        match family {
            NamespaceFamily::Spreadsheet => SPREADSHEET_NS,
            NamespaceFamily::Presentation => PRESENTATION_NS,
            NamespaceFamily::Wordprocessing => WORDPROCESSING_NS,
            NamespaceFamily::Drawing => DRAWING_NS,
            NamespaceFamily::Chart => CHART_NS,
        }
    }

    fn rule_xml(rule: &ChildRule) -> String {
        match rule.namespace {
            RuleNamespace::Parent => format!("<n:{}/>", rule.local_name),
            RuleNamespace::Drawing => format!("<a:{}/>", rule.local_name),
        }
    }

    fn table_xml(table: &SequenceTable, children: &str) -> String {
        format!(
            r#"<n:{root} xmlns:n="{namespace}" xmlns:a="{drawing}" xmlns:mc="{mc}" xmlns:f="urn:foreign">{children}</n:{root}>"#,
            root = table.parent_local_name,
            namespace = namespace_uri(table.parent_namespace),
            drawing = DRAWING_NS,
            mc = MC_NS,
        )
    }

    fn part_kind(table: &SequenceTable) -> CoveredPartKind {
        match table.parent_namespace {
            NamespaceFamily::Spreadsheet => CoveredPartKind::Spreadsheet,
            NamespaceFamily::Presentation => CoveredPartKind::Presentation,
            NamespaceFamily::Wordprocessing => CoveredPartKind::Wordprocessing,
            NamespaceFamily::Chart => CoveredPartKind::Chart,
            NamespaceFamily::Drawing => unreachable!("no drawing root sequence table"),
        }
    }

    #[test]
    fn every_sequence_table_accepts_declared_order() {
        for table in SEQUENCE_TABLES {
            let children = table.children.iter().map(rule_xml).collect::<String>();
            let diagnostics = validate_xml_part(
                &format!("/{}.xml", table.label),
                &table_xml(table, &children),
                part_kind(table),
            );
            assert!(
                diagnostics.is_empty(),
                "{} declared order should pass: {diagnostics:#?}",
                table.label
            );
        }
    }

    #[test]
    fn every_ordered_adjacent_pair_rejects_a_swap() {
        for table in SEQUENCE_TABLES {
            let ordered = table
                .children
                .iter()
                .filter(|rule| rule.position.is_some() && rule.local_name != "extLst")
                .collect::<Vec<_>>();
            for pair in ordered.windows(2) {
                if pair[0].position == pair[1].position {
                    continue;
                }
                let children = format!("{}{}", rule_xml(pair[1]), rule_xml(pair[0]));
                let diagnostics = validate_xml_part(
                    &format!("/{}.xml", table.label),
                    &table_xml(table, &children),
                    part_kind(table),
                );
                assert_eq!(diagnostics.len(), 1, "{} swapped pair", table.label);
                assert_eq!(diagnostics[0]["code"], "XML_CHILD_ORDER");
                assert_eq!(diagnostics[0]["element"], pair[0].local_name);
                assert_eq!(
                    diagnostics[0]["expectedPosition"],
                    pair[0].position.expect("ordered pair")
                );
            }
        }
    }

    #[test]
    fn every_sequence_table_rejects_an_unknown_schema_child() {
        for table in SEQUENCE_TABLES {
            let diagnostics = validate_xml_part(
                &format!("/{}.xml", table.label),
                &table_xml(table, "<n:notInTheSchema/>"),
                part_kind(table),
            );
            assert_eq!(diagnostics.len(), 1, "{} unknown child", table.label);
            assert_eq!(diagnostics[0]["code"], "XML_UNKNOWN_CHILD");
            assert_eq!(diagnostics[0]["element"], "notInTheSchema");
        }
    }

    #[test]
    fn extension_alternate_content_and_foreign_subtrees_are_permissive() {
        for table in SEQUENCE_TABLES {
            let children = r#"
                <n:extLst><n:notInTheSchema/></n:extLst>
                <mc:AlternateContent><mc:Choice Requires="f"><n:notInTheSchema/></mc:Choice></mc:AlternateContent>
                <f:notInTheSchema><n:notInTheSchema/></f:notInTheSchema>
            "#;
            let diagnostics = validate_xml_part(
                &format!("/{}.xml", table.label),
                &table_xml(table, children),
                part_kind(table),
            );
            assert!(
                diagnostics.is_empty(),
                "{} permissive wrappers: {diagnostics:#?}",
                table.label
            );
        }
    }
}
