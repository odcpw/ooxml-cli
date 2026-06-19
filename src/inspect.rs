use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Map, Value, json};

use crate::{
    CliError, CliResult, InspectPackageKind, attr, content_type_for_part,
    detect_inspect_package_type, find_docx_document_part, find_xlsx_workbook_part,
    is_custom_xml_part, is_docx_comments_part, is_docx_endnotes_part, is_docx_footer_part,
    is_docx_footnotes_part, is_docx_header_part, is_docx_media_part, is_docx_numbering_part,
    is_docx_styles_part, is_xlsx_chart_part, is_xlsx_media_part, is_xlsx_pivot_cache_part,
    is_xlsx_pivot_table_part, is_xlsx_shared_strings_part, is_xlsx_styles_part, is_xlsx_table_part,
    is_xlsx_theme_part, is_xlsx_worksheet_part, local_name, relationship_entries,
    relationships_part_for, resolve_relationship_target, stack_contains, workbook_sheets,
    zip_entry_names, zip_text,
};

pub(crate) fn inspect(file: &str) -> CliResult<Value> {
    let entries = zip_entry_names(file)?;
    match detect_inspect_package_type(file, &entries) {
        InspectPackageKind::Pptx => {
            let presentation = zip_text(file, "ppt/presentation.xml")?;
            let (cx, cy) = pptx_slide_size(&presentation)?;
            Ok(json!({
                "file": file,
                "summary": {
                    "customXmlParts": count_entries(&entries, "customXml/item", ".xml"),
                    "handoutMasters": count_entries(&entries, "ppt/handoutMasters/handoutMaster", ".xml"),
                    "layouts": count_entries(&entries, "ppt/slideLayouts/slideLayout", ".xml"),
                    "masters": count_entries(&entries, "ppt/slideMasters/slideMaster", ".xml"),
                    "mediaAssets": entries.iter().filter(|name| name.starts_with("ppt/media/")).count(),
                    "notesMasters": count_entries(&entries, "ppt/notesMasters/notesMaster", ".xml"),
                    "slideSize": {"cx": cx, "cy": cy, "unit": "emu"},
                    "slides": count_entries(&entries, "ppt/slides/slide", ".xml"),
                    "themes": count_entries(&entries, "ppt/theme/theme", ".xml"),
                },
                "type": "pptx",
            }))
        }
        InspectPackageKind::Xlsx => inspect_xlsx(file, &entries),
        InspectPackageKind::Docx => inspect_docx(file, &entries),
        InspectPackageKind::Unknown => Err(CliError::unsupported_type("unsupported type: unknown")),
    }
}

fn inspect_xlsx(file: &str, entries: &[String]) -> CliResult<Value> {
    let workbook_part = find_xlsx_workbook_part(file, entries)?;
    let workbook = zip_text(file, &workbook_part).map_err(|err| {
        CliError::unexpected(format!(
            "failed to inspect workbook: failed to read workbook part /{}: {}",
            workbook_part, err.message
        ))
    })?;
    let sheets = workbook_sheets(&workbook).map_err(|err| {
        if is_xml_parse_error(&err.message) {
            CliError::unexpected(format!(
                "failed to inspect workbook: failed to read workbook part /{}: failed to parse XML part /{}: {}",
                workbook_part,
                workbook_part,
                go_like_xml_parse_message(&err.message)
            ))
        } else {
            CliError::unexpected(format!(
                "failed to inspect workbook: workbook part /{} {}",
                workbook_part, err.message
            ))
        }
    })?;
    let workbook_rels =
        relationship_entries(file, &relationships_part_for(&workbook_part)).unwrap_or_default();
    let shared_strings_uri = workbook_rels
        .iter()
        .find(|rel| rel.rel_type.ends_with("/sharedStrings"))
        .map(|rel| resolve_relationship_target(&format!("/{workbook_part}"), &rel.target));
    let mut summary = Map::new();
    summary.insert("sheets".to_string(), json!(sheets.len()));
    summary.insert("worksheets".to_string(), json!(0));
    summary.insert("sharedStrings".to_string(), json!(false));
    summary.insert("styles".to_string(), json!(false));
    summary.insert("themes".to_string(), json!(0));
    summary.insert("tables".to_string(), json!(0));
    summary.insert("pivots".to_string(), json!(0));
    summary.insert("pivotCaches".to_string(), json!(0));
    summary.insert("charts".to_string(), json!(0));
    summary.insert("mediaAssets".to_string(), json!(0));
    summary.insert("customXmlParts".to_string(), json!(0));

    for entry in entries {
        let uri = format!("/{entry}");
        let content_type = content_type_for_part(file, entry).unwrap_or_default();
        if is_xlsx_worksheet_part(&uri, &content_type) {
            increment_json_count(&mut summary, "worksheets");
        } else if is_xlsx_shared_strings_part(&uri, &content_type) {
            summary.insert("sharedStrings".to_string(), json!(true));
        } else if is_xlsx_styles_part(&uri, &content_type) {
            summary.insert("styles".to_string(), json!(true));
        } else if is_xlsx_theme_part(&uri, &content_type) {
            increment_json_count(&mut summary, "themes");
        } else if is_xlsx_table_part(&uri, &content_type) {
            increment_json_count(&mut summary, "tables");
        } else if is_xlsx_pivot_table_part(&uri, &content_type) {
            increment_json_count(&mut summary, "pivots");
        } else if is_xlsx_pivot_cache_part(&uri, &content_type) {
            increment_json_count(&mut summary, "pivotCaches");
        } else if is_xlsx_chart_part(&uri, &content_type) {
            increment_json_count(&mut summary, "charts");
        } else if is_xlsx_media_part(&uri) {
            increment_json_count(&mut summary, "mediaAssets");
        } else if is_custom_xml_part(&uri) {
            increment_json_count(&mut summary, "customXmlParts");
        }
    }
    if let Some(shared_strings_uri) = shared_strings_uri {
        let count = shared_string_count(file, &shared_strings_uri).unwrap_or_default();
        if count > 0 {
            summary.insert("sharedStringCount".to_string(), json!(count));
        }
    }
    Ok(json!({
        "file": file,
        "summary": Value::Object(summary),
        "type": "xlsx",
    }))
}

fn inspect_docx(file: &str, entries: &[String]) -> CliResult<Value> {
    let document_part = find_docx_document_part(file, entries)?;
    let document = zip_text(file, &document_part).map_err(|_| {
        CliError::unexpected(format!(
            "failed to inspect document: failed to read document part /{}: part /{} not found",
            document_part, document_part
        ))
    })?;
    let counts = docx_body_summary_counts(&document).map_err(|err| {
        if is_xml_parse_error(&err) {
            CliError::unexpected(format!(
                "failed to inspect document: failed to read document part /{}: failed to parse XML part /{}: {}",
                document_part,
                document_part,
                go_like_docx_xml_parse_message(&err)
            ))
        } else {
            CliError::unexpected(format!(
                "failed to inspect document: document part /{} {}",
                document_part, err
            ))
        }
    })?;
    let mut summary = Map::new();
    summary.insert("paragraphs".to_string(), json!(counts.paragraphs));
    summary.insert("tables".to_string(), json!(counts.tables));
    summary.insert("hyperlinks".to_string(), json!(counts.hyperlinks));
    summary.insert("headers".to_string(), json!(0));
    summary.insert("footers".to_string(), json!(0));
    summary.insert("footnotes".to_string(), json!(false));
    summary.insert("endnotes".to_string(), json!(false));
    summary.insert("comments".to_string(), json!(false));
    summary.insert("sections".to_string(), json!(counts.sections));
    summary.insert("styles".to_string(), json!(false));
    summary.insert("numbering".to_string(), json!(false));
    summary.insert("mediaAssets".to_string(), json!(0));
    summary.insert("customXmlParts".to_string(), json!(0));

    for entry in entries {
        let uri = format!("/{entry}");
        let content_type = content_type_for_part(file, entry).unwrap_or_default();
        if is_docx_styles_part(&uri, &content_type) {
            summary.insert("styles".to_string(), json!(true));
        } else if is_docx_numbering_part(&uri, &content_type) {
            summary.insert("numbering".to_string(), json!(true));
        } else if is_docx_header_part(&uri, &content_type) {
            increment_json_count(&mut summary, "headers");
        } else if is_docx_footer_part(&uri, &content_type) {
            increment_json_count(&mut summary, "footers");
        } else if is_docx_footnotes_part(&uri, &content_type) {
            summary.insert("footnotes".to_string(), json!(true));
        } else if is_docx_endnotes_part(&uri, &content_type) {
            summary.insert("endnotes".to_string(), json!(true));
        } else if is_docx_comments_part(&uri, &content_type) {
            summary.insert("comments".to_string(), json!(true));
        } else if is_docx_media_part(&uri) {
            increment_json_count(&mut summary, "mediaAssets");
        } else if is_custom_xml_part(&uri) {
            increment_json_count(&mut summary, "customXmlParts");
        }
    }

    Ok(json!({
        "file": file,
        "summary": Value::Object(summary),
        "type": "docx",
    }))
}

fn is_xml_parse_error(message: &str) -> bool {
    message.contains("syntax error")
        || message.contains("unexpected EOF")
        || message.contains("not found before end of input")
}

fn go_like_xml_parse_message(message: &str) -> &'static str {
    if message.contains("unexpected EOF") || message.contains("not found before end of input") {
        "XML syntax error on line 1: unexpected EOF"
    } else {
        "XML syntax error"
    }
}

fn go_like_docx_xml_parse_message(message: &str) -> &'static str {
    if message.contains("unexpected EOF") {
        "etree: invalid XML format"
    } else {
        go_like_xml_parse_message(message)
    }
}

fn increment_json_count(summary: &mut Map<String, Value>, key: &str) {
    let next = summary.get(key).and_then(Value::as_u64).unwrap_or_default() + 1;
    summary.insert(key.to_string(), json!(next));
}

#[derive(Default)]
struct DocxBodySummaryCounts {
    paragraphs: usize,
    tables: usize,
    hyperlinks: usize,
    sections: usize,
}

fn docx_body_summary_counts(xml: &str) -> Result<DocxBodySummaryCounts, String> {
    let mut reader = Reader::from_str(xml);
    let mut stack: Vec<String> = Vec::new();
    let mut counts = DocxBodySummaryCounts::default();
    let mut direct_sections = 0usize;
    let mut descendant_sections = 0usize;
    let mut block_depth: Option<usize> = None;
    let mut saw_document = false;
    let mut saw_body = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if stack.is_empty() && name != "document" {
                    return Err(format!("root is {name:?}, expected document"));
                }
                if stack.is_empty() {
                    saw_document = true;
                }
                let parent = stack.last().map(String::as_str);
                if parent == Some("document") && name == "body" {
                    saw_body = true;
                }
                if parent == Some("body") && name == "p" {
                    counts.paragraphs += 1;
                    block_depth = Some(1);
                } else if parent == Some("body") && name == "tbl" {
                    counts.tables += 1;
                    block_depth = Some(1);
                } else if let Some(depth) = block_depth.as_mut() {
                    *depth += 1;
                }
                if name == "hyperlink" && block_depth.is_some() {
                    counts.hyperlinks += 1;
                }
                if name == "sectPr" && stack_contains(&stack, "body") {
                    descendant_sections += 1;
                    if parent == Some("body") {
                        direct_sections += 1;
                    }
                }
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if stack.is_empty() && name != "document" {
                    return Err(format!("root is {name:?}, expected document"));
                }
                if stack.is_empty() {
                    saw_document = true;
                }
                let parent = stack.last().map(String::as_str);
                if parent == Some("document") && name == "body" {
                    saw_body = true;
                }
                if parent == Some("body") && name == "p" {
                    counts.paragraphs += 1;
                } else if parent == Some("body") && name == "tbl" {
                    counts.tables += 1;
                }
                if name == "hyperlink" && block_depth.is_some() {
                    counts.hyperlinks += 1;
                }
                if name == "sectPr" && stack_contains(&stack, "body") {
                    descendant_sections += 1;
                    if parent == Some("body") {
                        direct_sections += 1;
                    }
                }
            }
            Ok(Event::End(_)) => {
                if let Some(depth) = block_depth.as_mut() {
                    *depth = depth.saturating_sub(1);
                    if *depth == 0 {
                        block_depth = None;
                    }
                }
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(err.to_string()),
            _ => {}
        }
    }
    if !stack.is_empty() {
        return Err("unexpected EOF".to_string());
    }
    if !saw_document {
        return Err("has no root element".to_string());
    }
    if !saw_body {
        return Err("body element not found".to_string());
    }
    counts.sections = if direct_sections > 0 {
        direct_sections
    } else {
        descendant_sections
    };
    Ok(counts)
}

fn count_entries(entries: &[String], prefix: &str, suffix: &str) -> usize {
    entries
        .iter()
        .filter(|name| {
            name.starts_with(prefix)
                && name.ends_with(suffix)
                && !name.contains("/_rels/")
                && !name.ends_with(".rels")
        })
        .count()
}

fn pptx_slide_size(xml: &str) -> CliResult<(i64, i64)> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "sldSz" =>
            {
                let cx = attr(&e, "cx")
                    .and_then(|v| v.parse::<i64>().ok())
                    .ok_or_else(|| CliError::unexpected("presentation slide size missing cx"))?;
                let cy = attr(&e, "cy")
                    .and_then(|v| v.parse::<i64>().ok())
                    .ok_or_else(|| CliError::unexpected("presentation slide size missing cy"))?;
                return Ok((cx, cy));
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Err(CliError::unexpected("presentation slide size not found"))
}

fn shared_string_count(file: &str, part_uri: &str) -> CliResult<usize> {
    let xml = zip_text(file, part_uri.trim_start_matches('/'))?;
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);
    let mut saw_root = false;
    let mut count = 0usize;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if !saw_root {
                    if name != "sst" {
                        return Err(CliError::unexpected(
                            "shared string table root element not found",
                        ));
                    }
                    saw_root = true;
                } else if name == "si" {
                    count += 1;
                }
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if !saw_root {
                    if name != "sst" {
                        return Err(CliError::unexpected(
                            "shared string table root element not found",
                        ));
                    }
                    saw_root = true;
                } else if name == "si" {
                    count += 1;
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    if saw_root {
        Ok(count)
    } else {
        Err(CliError::unexpected(
            "shared string table root element not found",
        ))
    }
}
