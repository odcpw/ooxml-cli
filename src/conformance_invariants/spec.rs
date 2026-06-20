use serde_json::Value;

use super::util::{base_name, diag, is_rels_uri, normalize_uri};
pub(super) const CONTENT_TYPES_PART_URI: &str = "/[Content_Types].xml";
pub(super) const CONTENT_TYPES_NAMESPACE: &str =
    "http://schemas.openxmlformats.org/package/2006/content-types";
pub(super) const SPREADSHEETML_NAMESPACE: &str =
    "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
pub(super) const PRESENTATIONML_NAMESPACE: &str =
    "http://schemas.openxmlformats.org/presentationml/2006/main";
pub(super) const WORDPROCESSINGML_NAMESPACE: &str =
    "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
pub(super) const SPREADSHEET_DRAWING_NAMESPACE: &str =
    "http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing";
pub(super) const CHART_NAMESPACE: &str = "http://schemas.openxmlformats.org/drawingml/2006/chart";

pub(super) const CONTENT_TYPE_RELS: &str =
    "application/vnd.openxmlformats-package.relationships+xml";
pub(super) const CONTENT_TYPE_XLSX_WORKBOOK: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml";
pub(super) const CONTENT_TYPE_XLSX_WORKBOOK_MACRO: &str =
    "application/vnd.ms-excel.sheet.macroEnabled.main+xml";
pub(super) const CONTENT_TYPE_XLSX_WORKBOOK_TEMPLATE: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.template.main+xml";
pub(super) const CONTENT_TYPE_XLSX_WORKBOOK_ADDIN: &str =
    "application/vnd.ms-excel.addin.macroEnabled.main+xml";
pub(super) const CONTENT_TYPE_XLSX_SHARED_STRINGS: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml";
pub(super) const CONTENT_TYPE_XLSX_STYLES: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml";
pub(super) const CONTENT_TYPE_XLSX_CALC_CHAIN: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.calcChain+xml";
pub(super) const CONTENT_TYPE_XLSX_WORKSHEET: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml";
pub(super) const CONTENT_TYPE_XLSX_CHARTSHEET: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.chartsheet+xml";
pub(super) const CONTENT_TYPE_XLSX_DIALOGSHEET: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.dialogsheet+xml";
pub(super) const CONTENT_TYPE_XLSX_TABLE: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.table+xml";
pub(super) const CONTENT_TYPE_XLSX_PIVOT_TABLE: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.pivotTable+xml";
pub(super) const CONTENT_TYPE_XLSX_PIVOT_CACHE: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.pivotCacheDefinition+xml";
pub(super) const CONTENT_TYPE_XLSX_PIVOT_RECORDS: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.pivotCacheRecords+xml";
pub(super) const CONTENT_TYPE_XLSX_COMMENTS: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.comments+xml";
pub(super) const CONTENT_TYPE_XLSX_VML: &str =
    "application/vnd.openxmlformats-officedocument.vmlDrawing";
pub(super) const CONTENT_TYPE_DRAWING: &str =
    "application/vnd.openxmlformats-officedocument.drawing+xml";
pub(super) const CONTENT_TYPE_CHART: &str =
    "application/vnd.openxmlformats-officedocument.drawingml.chart+xml";

pub(super) const CONTENT_TYPE_PPTX_PRESENTATION: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml";
pub(super) const CONTENT_TYPE_PPTX_PRESENTATION_MACRO: &str =
    "application/vnd.ms-powerpoint.presentation.macroEnabled.main+xml";
pub(super) const CONTENT_TYPE_PPTX_TEMPLATE: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.template.main+xml";
pub(super) const CONTENT_TYPE_PPTX_TEMPLATE_MACRO: &str =
    "application/vnd.ms-powerpoint.template.macroEnabled.main+xml";
pub(super) const CONTENT_TYPE_PPTX_SLIDESHOW: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.slideshow.main+xml";
pub(super) const CONTENT_TYPE_PPTX_SLIDESHOW_MACRO: &str =
    "application/vnd.ms-powerpoint.slideshow.macroEnabled.main+xml";
pub(super) const CONTENT_TYPE_PPTX_SLIDE: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.slide+xml";
pub(super) const CONTENT_TYPE_PPTX_SLIDE_LAYOUT: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml";
pub(super) const CONTENT_TYPE_PPTX_SLIDE_MASTER: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml";
pub(super) const CONTENT_TYPE_PPTX_NOTES_SLIDE: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.notesSlide+xml";
pub(super) const CONTENT_TYPE_PPTX_NOTES_MASTER: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.notesMaster+xml";
pub(super) const CONTENT_TYPE_PPTX_THEME: &str =
    "application/vnd.openxmlformats-officedocument.theme+xml";
pub(super) const CONTENT_TYPE_PPTX_TABLE_STYLES: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.tableStyles+xml";
pub(super) const CONTENT_TYPE_PPTX_COMMENTS: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.comments+xml";
pub(super) const CONTENT_TYPE_PPTX_COMMENT_AUTHORS: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.commentAuthors+xml";
pub(super) const CONTENT_TYPE_PPTX_PRES_PROPS: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.presProps+xml";
pub(super) const CONTENT_TYPE_PPTX_VIEW_PROPS: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.viewProps+xml";

pub(super) const CONTENT_TYPE_DOCX_DOCUMENT: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml";
pub(super) const CONTENT_TYPE_DOCX_DOCUMENT_MACRO: &str =
    "application/vnd.ms-word.document.macroEnabled.main+xml";
pub(super) const CONTENT_TYPE_DOCX_TEMPLATE: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.template.main+xml";
pub(super) const CONTENT_TYPE_DOCX_TEMPLATE_MACRO: &str =
    "application/vnd.ms-word.template.macroEnabledTemplate.main+xml";
pub(super) const CONTENT_TYPE_DOCX_STYLES: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml";
pub(super) const CONTENT_TYPE_DOCX_NUMBERING: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml";
pub(super) const CONTENT_TYPE_DOCX_FOOTNOTES: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml";
pub(super) const CONTENT_TYPE_DOCX_ENDNOTES: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.endnotes+xml";
pub(super) const CONTENT_TYPE_DOCX_COMMENTS: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml";
pub(super) const CONTENT_TYPE_DOCX_HEADER: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml";
pub(super) const CONTENT_TYPE_DOCX_FOOTER: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml";

pub(super) const REL_TYPE_OFFICE_DOCUMENT: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument";
pub(super) const REL_TYPE_XLSX_WORKSHEET: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet";
pub(super) const REL_TYPE_XLSX_CHARTSHEET: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/chartsheet";
pub(super) const REL_TYPE_XLSX_DIALOGSHEET: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/dialogsheet";
pub(super) const REL_TYPE_XLSX_SHARED_STRINGS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings";
pub(super) const REL_TYPE_STYLES: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles";
pub(super) const REL_TYPE_DOCX_NUMBERING: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering";
pub(super) const REL_TYPE_DOCX_HEADER: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/header";
pub(super) const REL_TYPE_DOCX_FOOTER: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer";
pub(super) const REL_TYPE_DOCX_FOOTNOTES: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes";
pub(super) const REL_TYPE_DOCX_ENDNOTES: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/endnotes";
pub(super) const REL_TYPE_XLSX_CALC_CHAIN: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/calcChain";
pub(super) const REL_TYPE_XLSX_TABLE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/table";
pub(super) const REL_TYPE_XLSX_DRAWING: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/drawing";
pub(super) const REL_TYPE_CHART: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/chart";
pub(super) const REL_TYPE_XLSX_PIVOT_TABLE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotTable";
pub(super) const REL_TYPE_XLSX_PIVOT_CACHE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotCacheDefinition";
pub(super) const REL_TYPE_XLSX_PIVOT_RECORDS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotCacheRecords";
pub(super) const REL_TYPE_COMMENTS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments";
pub(super) const REL_TYPE_PPTX_SLIDE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide";
pub(super) const REL_TYPE_PPTX_SLIDE_LAYOUT: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout";
pub(super) const REL_TYPE_PPTX_SLIDE_MASTER: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster";
pub(super) const REL_TYPE_PPTX_NOTES_SLIDE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesSlide";
pub(super) const REL_TYPE_PPTX_NOTES_MASTER: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesMaster";
pub(super) const REL_TYPE_OFFICE_THEME: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme";
pub(super) const REL_TYPE_PPTX_COMMENT_AUTHORS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/commentAuthors";

pub(super) fn check_known_part_content_type(part_uri: &str, content_type: &str) -> Vec<Value> {
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

pub(super) fn expected_relationship_target_content_types(
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

pub(super) fn is_xlsx_workbook_content_type(content_type: &str) -> bool {
    matches!(
        content_type,
        CONTENT_TYPE_XLSX_WORKBOOK
            | CONTENT_TYPE_XLSX_WORKBOOK_MACRO
            | CONTENT_TYPE_XLSX_WORKBOOK_TEMPLATE
            | CONTENT_TYPE_XLSX_WORKBOOK_ADDIN
    )
}

pub(super) fn is_pptx_presentation_content_type(content_type: &str) -> bool {
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
