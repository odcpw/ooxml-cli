use crate::{
    CliError, CliResult, content_type_for_part, relationship_entries, resolve_relationship_target,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum InspectPackageKind {
    Pptx,
    Xlsx,
    Docx,
    Unknown,
}

pub(crate) fn detect_inspect_package_type(file: &str, entries: &[String]) -> InspectPackageKind {
    for rel in relationship_entries(file, "_rels/.rels").unwrap_or_default() {
        let target_uri = resolve_relationship_target("/", &rel.target);
        let target_content_type = content_type_for_part(file, &target_uri).unwrap_or_default();

        if rel.rel_type.contains("presentationml.presentation") {
            return InspectPackageKind::Pptx;
        }
        if target_content_type.contains("presentationml.presentation")
            || target_uri.starts_with("/ppt/")
        {
            return InspectPackageKind::Pptx;
        }

        if rel.rel_type.contains("wordprocessingml.document") {
            return InspectPackageKind::Docx;
        }
        if target_content_type.contains("wordprocessingml.document")
            || target_uri.starts_with("/word/")
        {
            return InspectPackageKind::Docx;
        }

        if rel.rel_type.contains("spreadsheetml.sheet") {
            return InspectPackageKind::Xlsx;
        }
        if target_content_type.contains("spreadsheetml.sheet") || target_uri.starts_with("/xl/") {
            return InspectPackageKind::Xlsx;
        }
    }

    for entry in entries {
        let content_type = content_type_for_part(file, entry).unwrap_or_default();
        if content_type.contains("presentationml") {
            return InspectPackageKind::Pptx;
        }
        if content_type.contains("wordprocessingml") {
            return InspectPackageKind::Docx;
        }
        if content_type.contains("spreadsheetml") {
            return InspectPackageKind::Xlsx;
        }
    }

    InspectPackageKind::Unknown
}

pub(crate) fn find_xlsx_workbook_part(file: &str, entries: &[String]) -> CliResult<String> {
    for rel in relationship_entries(file, "_rels/.rels").unwrap_or_default() {
        if rel.target_mode == "External" {
            continue;
        }
        let target = resolve_relationship_target("/", &rel.target);
        if is_xlsx_workbook_candidate(file, &target) {
            return Ok(target.trim_start_matches('/').to_string());
        }
    }
    for entry in entries {
        let content_type = content_type_for_part(file, entry).unwrap_or_default();
        if is_xlsx_workbook_content_type(&content_type) {
            return Ok(entry.clone());
        }
    }
    Err(CliError::unexpected("xlsx workbook part not found"))
}

fn is_xlsx_workbook_candidate(file: &str, uri: &str) -> bool {
    if uri.is_empty() || uri == "/" {
        return false;
    }
    let content_type = content_type_for_part(file, uri).unwrap_or_default();
    is_xlsx_workbook_content_type(&content_type) || uri == "/xl/workbook.xml"
}

fn is_xlsx_workbook_content_type(content_type: &str) -> bool {
    matches!(
        content_type,
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"
            | "application/vnd.ms-excel.sheet.macroEnabled.main+xml"
            | "application/vnd.openxmlformats-officedocument.spreadsheetml.template.main+xml"
            | "application/vnd.ms-excel.addin.macroEnabled.main+xml"
    ) || content_type.contains("spreadsheetml.sheet.main+xml")
        || content_type.contains("spreadsheetml.template.main+xml")
        || content_type.contains("ms-excel.sheet.macroEnabled.main+xml")
        || content_type.contains("ms-excel.addin.macroEnabled.main+xml")
}

pub(crate) fn find_docx_document_part(file: &str, entries: &[String]) -> CliResult<String> {
    for rel in relationship_entries(file, "_rels/.rels").unwrap_or_default() {
        if rel.target_mode == "External" {
            continue;
        }
        let target = resolve_relationship_target("/", &rel.target);
        if rel.rel_type.ends_with("/officeDocument") || is_docx_document_candidate(file, &target) {
            return Ok(target.trim_start_matches('/').to_string());
        }
    }
    for entry in entries {
        let content_type = content_type_for_part(file, entry).unwrap_or_default();
        if is_docx_document_content_type(&content_type) {
            return Ok(entry.clone());
        }
    }
    Err(CliError::unexpected("docx main document part not found"))
}

fn is_docx_document_candidate(file: &str, uri: &str) -> bool {
    if uri.is_empty() || uri == "/" {
        return false;
    }
    let content_type = content_type_for_part(file, uri).unwrap_or_default();
    is_docx_document_content_type(&content_type) || uri == "/word/document.xml"
}

fn is_docx_document_content_type(content_type: &str) -> bool {
    content_type
        == "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"
        || content_type.contains("wordprocessingml.document.main+xml")
}

pub(crate) fn is_xlsx_worksheet_part(uri: &str, content_type: &str) -> bool {
    content_type == "application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"
        || is_xml_data_part(uri) && uri.starts_with("/xl/worksheets/")
}

pub(crate) fn is_xlsx_shared_strings_part(uri: &str, content_type: &str) -> bool {
    content_type == "application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml"
        || uri == "/xl/sharedStrings.xml"
}

pub(crate) fn is_xlsx_styles_part(uri: &str, content_type: &str) -> bool {
    content_type == "application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"
        || uri == "/xl/styles.xml"
}

pub(crate) fn is_xlsx_theme_part(uri: &str, content_type: &str) -> bool {
    content_type == "application/vnd.openxmlformats-officedocument.theme+xml"
        || is_xml_data_part(uri) && uri.starts_with("/xl/theme/")
}

pub(crate) fn is_xlsx_table_part(uri: &str, content_type: &str) -> bool {
    content_type == "application/vnd.openxmlformats-officedocument.spreadsheetml.table+xml"
        || is_xml_data_part(uri) && uri.starts_with("/xl/tables/")
}

pub(crate) fn is_xlsx_pivot_table_part(uri: &str, content_type: &str) -> bool {
    content_type == "application/vnd.openxmlformats-officedocument.spreadsheetml.pivotTable+xml"
        || is_xml_data_part(uri) && uri.starts_with("/xl/pivotTables/")
}

pub(crate) fn is_xlsx_pivot_cache_part(uri: &str, content_type: &str) -> bool {
    content_type
        == "application/vnd.openxmlformats-officedocument.spreadsheetml.pivotCacheDefinition+xml"
        || is_xml_data_part(uri)
            && uri.starts_with("/xl/pivotCache/")
            && file_name(uri).starts_with("pivotCacheDefinition")
}

pub(crate) fn is_xlsx_chart_part(uri: &str, content_type: &str) -> bool {
    content_type == "application/vnd.openxmlformats-officedocument.drawingml.chart+xml"
        || is_xml_data_part(uri)
            && uri.starts_with("/xl/charts/")
            && file_name(uri).starts_with("chart")
}

pub(crate) fn is_xlsx_media_part(uri: &str) -> bool {
    uri.starts_with("/xl/media/") && !uri.contains("/_rels/")
}

pub(crate) fn is_docx_styles_part(uri: &str, content_type: &str) -> bool {
    content_type == "application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"
        || uri == "/word/styles.xml"
}

pub(crate) fn is_docx_numbering_part(uri: &str, content_type: &str) -> bool {
    content_type == "application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml"
        || uri == "/word/numbering.xml"
}

pub(crate) fn is_docx_header_part(uri: &str, content_type: &str) -> bool {
    content_type == "application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"
        || is_xml_data_part(uri) && uri.starts_with("/word/header")
}

pub(crate) fn is_docx_footer_part(uri: &str, content_type: &str) -> bool {
    content_type == "application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"
        || is_xml_data_part(uri) && uri.starts_with("/word/footer")
}

pub(crate) fn is_docx_footnotes_part(uri: &str, content_type: &str) -> bool {
    content_type == "application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"
        || uri == "/word/footnotes.xml"
}

pub(crate) fn is_docx_endnotes_part(uri: &str, content_type: &str) -> bool {
    content_type == "application/vnd.openxmlformats-officedocument.wordprocessingml.endnotes+xml"
        || uri == "/word/endnotes.xml"
}

pub(crate) fn is_docx_comments_part(uri: &str, content_type: &str) -> bool {
    content_type == "application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"
        || uri == "/word/comments.xml"
}

pub(crate) fn is_docx_media_part(uri: &str) -> bool {
    uri.starts_with("/word/media/") && !uri.contains("/_rels/")
}

pub(crate) fn is_custom_xml_part(uri: &str) -> bool {
    is_xml_data_part(uri) && uri.starts_with("/customXml/")
}

fn is_xml_data_part(uri: &str) -> bool {
    uri.ends_with(".xml") && !uri.contains("/_rels/") && !uri.ends_with(".rels")
}

fn file_name(uri: &str) -> &str {
    uri.rsplit('/').next().unwrap_or(uri)
}
