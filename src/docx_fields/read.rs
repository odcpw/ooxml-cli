use quick_xml::NsReader;
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::NamespaceResolver;
use serde_json::{Map, Value, json};

use super::{docx_field_code_base, docx_field_location};
use crate::{
    CliError, CliResult, DOCX_W_NS, InspectPackageKind, append_xml_text_event,
    detect_inspect_package_type, docx_header_footer_part_uris, docx_word_attr_ns, element_in_ns,
    find_docx_document_part, is_xml_text_event, local_name, package_type, zip_entry_names,
    zip_text,
};
pub(crate) fn docx_fields_list(file: &str, type_filter: Option<&str>) -> CliResult<Value> {
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
            "failed to list fields: failed to read document part {document_uri}: {}",
            err.message
        ))
    })?;
    let mut fields = docx_fields_in_document_xml(&xml, &document_uri)
        .map_err(|err| CliError::unexpected(format!("failed to list fields: {}", err.message)))?;

    for part_uri in docx_header_footer_part_uris(file, &document_part, &document_uri, &xml)? {
        let part_xml = match zip_text(file, part_uri.trim_start_matches('/')) {
            Ok(part_xml) => part_xml,
            Err(_) => continue,
        };
        fields.extend(
            docx_fields_in_header_footer_xml(&part_xml, &part_uri).map_err(|err| {
                CliError::unexpected(format!("failed to list fields: {}", err.message))
            })?,
        );
    }

    for (index, field) in fields.iter_mut().enumerate() {
        field.index = index;
    }
    if let Some(type_filter) = type_filter.filter(|value| !value.is_empty()) {
        let wanted = type_filter.to_ascii_uppercase();
        fields.retain(|field| docx_field_code_base(&field.instruction) == wanted);
    }
    let fields = fields.iter().map(docx_field_json).collect::<Vec<_>>();
    Ok(json!({
        "file": file,
        "documentPartUri": document_uri,
        "fields": fields,
    }))
}

#[derive(Clone, Default)]
struct DocxFieldInfo {
    index: usize,
    part_uri: String,
    block_index: usize,
    block_kind: String,
    field_type: String,
    instruction: String,
    cached_result: String,
    location: String,
    editable: bool,
}

#[derive(Default)]
struct DocxFieldParagraphState {
    part_uri: String,
    block_index: usize,
    block_kind: String,
    location: String,
    editable: bool,
    simple: Option<DocxSimpleFieldState>,
    complex: DocxComplexFieldState,
}

#[derive(Default)]
struct DocxSimpleFieldState {
    instruction: String,
    result: String,
    depth: usize,
    in_t: bool,
}

#[derive(Default)]
struct DocxComplexFieldState {
    in_field: bool,
    after_separator: bool,
    depth: usize,
    instruction: String,
    result: String,
    in_instruction_text: bool,
    in_result_text: bool,
}

fn docx_fields_in_document_xml(xml: &str, document_uri: &str) -> CliResult<Vec<DocxFieldInfo>> {
    let mut reader = NsReader::from_str(xml);
    let mut stack: Vec<String> = Vec::new();
    let mut fields = Vec::new();
    let mut current: Option<DocxFieldParagraphState> = None;
    let mut body_block_index = 0usize;
    let mut body_table_depth = 0usize;
    let mut current_table_block = 0usize;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let parent = stack.last().map(String::as_str);
                let is_word = element_in_ns(reader.resolver(), &e, DOCX_W_NS);

                if parent == Some("body") && is_word && name == "p" {
                    body_block_index += 1;
                    current = Some(docx_field_paragraph_state(
                        document_uri,
                        body_block_index,
                        "paragraph",
                        true,
                    ));
                } else if parent == Some("body") && is_word && name == "tbl" {
                    body_block_index += 1;
                    current_table_block = body_block_index;
                    body_table_depth = 1;
                } else if body_table_depth > 0 && is_word && name == "tbl" {
                    body_table_depth += 1;
                } else if body_table_depth > 0
                    && is_word
                    && name == "p"
                    && stack.iter().any(|item| item == "tc")
                {
                    current = Some(docx_field_paragraph_state(
                        document_uri,
                        current_table_block,
                        "table",
                        false,
                    ));
                }

                if let Some(paragraph) = current.as_mut() {
                    docx_field_note_start(paragraph, &e, reader.resolver(), parent, is_word);
                }
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let parent = stack.last().map(String::as_str);
                let is_word = element_in_ns(reader.resolver(), &e, DOCX_W_NS);
                if parent == Some("body") && is_word && matches!(name.as_str(), "p" | "tbl") {
                    body_block_index += 1;
                } else if let Some(paragraph) = current.as_mut() {
                    docx_field_note_empty(
                        paragraph,
                        &e,
                        reader.resolver(),
                        parent,
                        is_word,
                        &mut fields,
                    );
                }
            }
            Ok(event) if is_xml_text_event(&event) => {
                if let Some(paragraph) = current.as_mut() {
                    let mut text = String::new();
                    append_xml_text_event(&mut text, &event);
                    docx_field_note_text(paragraph, &text);
                }
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if let Some(paragraph) = current.as_mut() {
                    docx_field_note_end(paragraph, &name, &mut fields);
                }
                if name == "p" {
                    current = None;
                } else if name == "tbl" {
                    body_table_depth = body_table_depth.saturating_sub(1);
                    if body_table_depth == 0 {
                        current_table_block = 0;
                    }
                }
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(fields)
}

fn docx_fields_in_header_footer_xml(xml: &str, part_uri: &str) -> CliResult<Vec<DocxFieldInfo>> {
    let mut reader = NsReader::from_str(xml);
    let mut stack: Vec<String> = Vec::new();
    let mut fields = Vec::new();
    let mut current: Option<DocxFieldParagraphState> = None;
    let mut paragraph_index = 0usize;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let parent = stack.last().map(String::as_str);
                let is_word = element_in_ns(reader.resolver(), &e, DOCX_W_NS);
                if stack.len() == 1 && is_word && name == "p" {
                    paragraph_index += 1;
                    current = Some(docx_field_paragraph_state(
                        part_uri,
                        paragraph_index,
                        "paragraph",
                        true,
                    ));
                }
                if let Some(paragraph) = current.as_mut() {
                    docx_field_note_start(paragraph, &e, reader.resolver(), parent, is_word);
                }
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let parent = stack.last().map(String::as_str);
                let is_word = element_in_ns(reader.resolver(), &e, DOCX_W_NS);
                if stack.len() == 1 && is_word && name == "p" {
                    paragraph_index += 1;
                } else if let Some(paragraph) = current.as_mut() {
                    docx_field_note_empty(
                        paragraph,
                        &e,
                        reader.resolver(),
                        parent,
                        is_word,
                        &mut fields,
                    );
                }
            }
            Ok(event) if is_xml_text_event(&event) => {
                if let Some(paragraph) = current.as_mut() {
                    let mut text = String::new();
                    append_xml_text_event(&mut text, &event);
                    docx_field_note_text(paragraph, &text);
                }
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if let Some(paragraph) = current.as_mut() {
                    docx_field_note_end(paragraph, &name, &mut fields);
                }
                if name == "p" {
                    current = None;
                }
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(fields)
}

fn docx_field_paragraph_state(
    part_uri: &str,
    block_index: usize,
    block_kind: &str,
    editable: bool,
) -> DocxFieldParagraphState {
    DocxFieldParagraphState {
        part_uri: part_uri.to_string(),
        block_index,
        block_kind: block_kind.to_string(),
        location: docx_field_location(part_uri, block_index),
        editable,
        ..DocxFieldParagraphState::default()
    }
}

fn docx_field_note_start(
    paragraph: &mut DocxFieldParagraphState,
    element: &BytesStart<'_>,
    resolver: &NamespaceResolver,
    parent: Option<&str>,
    is_word: bool,
) {
    let qualified_name = element.name();
    let name = local_name(qualified_name.as_ref());

    if paragraph.simple.is_none() && is_word && parent == Some("p") && name == "fldSimple" {
        paragraph.simple = Some(DocxSimpleFieldState {
            instruction: docx_word_attr_ns(element, resolver, b"instr").unwrap_or_default(),
            depth: 1,
            ..DocxSimpleFieldState::default()
        });
        return;
    }

    if let Some(simple) = paragraph.simple.as_mut() {
        simple.depth += 1;
        if is_word && name == "t" && element_in_ns(resolver, element, DOCX_W_NS) {
            simple.in_t = true;
        }
        return;
    }

    if !is_word {
        return;
    }

    docx_field_note_complex_start(&mut paragraph.complex, element, resolver);
}

fn docx_field_note_empty(
    paragraph: &mut DocxFieldParagraphState,
    element: &BytesStart<'_>,
    resolver: &NamespaceResolver,
    parent: Option<&str>,
    is_word: bool,
    fields: &mut Vec<DocxFieldInfo>,
) {
    let qualified_name = element.name();
    let name = local_name(qualified_name.as_ref());
    if !is_word {
        return;
    }
    if paragraph.simple.is_none() && parent == Some("p") && name == "fldSimple" {
        paragraph.simple = Some(DocxSimpleFieldState {
            instruction: docx_word_attr_ns(element, resolver, b"instr").unwrap_or_default(),
            depth: 1,
            ..DocxSimpleFieldState::default()
        });
        docx_emit_simple_field(paragraph, fields);
        return;
    }
    if paragraph.simple.is_some() {
        if name == "t" {
            // Empty w:t contributes no text but still belongs to the current simple field.
        }
        return;
    }
    let field_char_type = if name == "fldChar" {
        docx_word_attr_ns(element, resolver, b"fldCharType")
    } else {
        None
    };
    docx_field_note_complex_start(&mut paragraph.complex, element, resolver);
    if field_char_type.as_deref() == Some("end")
        && paragraph.complex.in_field
        && paragraph.complex.depth == 0
    {
        docx_emit_complex_field(paragraph, fields);
    }
}

fn docx_field_note_complex_start(
    complex: &mut DocxComplexFieldState,
    element: &BytesStart<'_>,
    resolver: &NamespaceResolver,
) {
    let qualified_name = element.name();
    let name = local_name(qualified_name.as_ref());
    if !element_in_ns(resolver, element, DOCX_W_NS) {
        return;
    }
    match name {
        "fldChar" => {
            let field_char_type =
                docx_word_attr_ns(element, resolver, b"fldCharType").unwrap_or_default();
            match field_char_type.as_str() {
                "begin" => {
                    if !complex.in_field {
                        complex.in_field = true;
                        complex.after_separator = false;
                        complex.depth = 1;
                        complex.instruction.clear();
                        complex.result.clear();
                    } else {
                        complex.depth += 1;
                    }
                }
                "separate" => {
                    if complex.in_field && complex.depth == 1 {
                        complex.after_separator = true;
                    }
                }
                "end" if complex.in_field => {
                    complex.depth = complex.depth.saturating_sub(1);
                }
                _ => {}
            }
        }
        "instrText" if complex.in_field && complex.depth == 1 && !complex.after_separator => {
            complex.in_instruction_text = true;
        }
        "t" if complex.in_field && complex.depth == 1 && complex.after_separator => {
            complex.in_result_text = true;
        }
        _ => {}
    }
}

fn docx_field_note_text(paragraph: &mut DocxFieldParagraphState, text: &str) {
    if let Some(simple) = paragraph.simple.as_mut() {
        if simple.in_t {
            simple.result.push_str(text);
        }
        return;
    }
    let complex = &mut paragraph.complex;
    if complex.in_instruction_text {
        complex.instruction.push_str(text);
    } else if complex.in_result_text {
        complex.result.push_str(text);
    }
}

fn docx_field_note_end(
    paragraph: &mut DocxFieldParagraphState,
    name: &str,
    fields: &mut Vec<DocxFieldInfo>,
) {
    if let Some(simple) = paragraph.simple.as_mut() {
        if name == "t" {
            simple.in_t = false;
        }
        if simple.depth > 0 {
            simple.depth -= 1;
        }
        if simple.depth == 0 {
            docx_emit_simple_field(paragraph, fields);
        }
        return;
    }

    let complex = &mut paragraph.complex;
    match name {
        "instrText" => complex.in_instruction_text = false,
        "t" => complex.in_result_text = false,
        "fldChar" if complex.in_field && complex.depth == 0 => {
            docx_emit_complex_field(paragraph, fields);
        }
        _ => {}
    }
}

fn docx_emit_simple_field(
    paragraph: &mut DocxFieldParagraphState,
    fields: &mut Vec<DocxFieldInfo>,
) {
    let Some(simple) = paragraph.simple.take() else {
        return;
    };
    fields.push(docx_field_info(
        paragraph,
        "simple",
        simple.instruction.trim(),
        &simple.result,
    ));
}

fn docx_emit_complex_field(
    paragraph: &mut DocxFieldParagraphState,
    fields: &mut Vec<DocxFieldInfo>,
) {
    let instruction = paragraph.complex.instruction.trim().to_string();
    let cached_result = paragraph.complex.result.clone();
    fields.push(docx_field_info(
        paragraph,
        "complex",
        &instruction,
        &cached_result,
    ));
    paragraph.complex = DocxComplexFieldState::default();
}

fn docx_field_info(
    paragraph: &DocxFieldParagraphState,
    field_type: &str,
    instruction: &str,
    cached_result: &str,
) -> DocxFieldInfo {
    DocxFieldInfo {
        part_uri: paragraph.part_uri.clone(),
        block_index: paragraph.block_index,
        block_kind: paragraph.block_kind.clone(),
        field_type: field_type.to_string(),
        instruction: instruction.to_string(),
        cached_result: cached_result.to_string(),
        location: paragraph.location.clone(),
        editable: paragraph.editable,
        ..DocxFieldInfo::default()
    }
}

fn docx_field_json(field: &DocxFieldInfo) -> Value {
    let mut object = Map::new();
    object.insert("index".to_string(), json!(field.index));
    object.insert("partUri".to_string(), json!(field.part_uri));
    object.insert("blockIndex".to_string(), json!(field.block_index));
    object.insert("blockKind".to_string(), json!(field.block_kind));
    object.insert("fieldType".to_string(), json!(field.field_type));
    object.insert("instruction".to_string(), json!(field.instruction));
    object.insert("cachedResult".to_string(), json!(field.cached_result));
    object.insert("location".to_string(), json!(field.location));
    object.insert("isStale".to_string(), json!(true));
    object.insert("editable".to_string(), json!(field.editable));
    Value::Object(object)
}
