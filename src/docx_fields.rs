use quick_xml::NsReader;
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::NamespaceResolver;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

use crate::{
    CliError, CliResult, DOCX_W_NS, DocxParagraphMutationOptions, InspectPackageKind,
    XmlNamedRange, XmlRange, append_docx_text_children, decode_xml_text,
    detect_inspect_package_type, docx_body_block_ranges, docx_body_tag, docx_first_word_attr,
    docx_header_footer_part_uris, docx_header_footer_root_tag,
    docx_mutation_output_path_for_result, docx_paragraph_fragment_text,
    docx_validate_strict_command, docx_word_attr_ns, docx_word_text_descendants, element_in_ns,
    ensure_docx_package_kind, find_docx_document_part, local_name, package_type,
    validate_xlsx_mutation_output_flags, word_xml_tag, write_docx_package_mutation_output,
    xml_attr_escape, xml_direct_child_ranges, xml_fragment_bounds, xml_fragment_text,
    xml_general_ref, xml_open_tag_from_start, xml_tag_prefix, zip_entry_names, zip_text,
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
            Ok(Event::Text(e)) => {
                if let Some(paragraph) = current.as_mut() {
                    docx_field_note_text(paragraph, &decode_xml_text(e.as_ref()));
                }
            }
            Ok(Event::GeneralRef(e)) => {
                if let Some(paragraph) = current.as_mut() {
                    docx_field_note_text(paragraph, &xml_general_ref(e.as_ref()));
                }
            }
            Ok(Event::CData(e)) => {
                if let Some(paragraph) = current.as_mut() {
                    docx_field_note_text(paragraph, &String::from_utf8_lossy(e.as_ref()));
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
            Ok(Event::Text(e)) => {
                if let Some(paragraph) = current.as_mut() {
                    docx_field_note_text(paragraph, &decode_xml_text(e.as_ref()));
                }
            }
            Ok(Event::GeneralRef(e)) => {
                if let Some(paragraph) = current.as_mut() {
                    docx_field_note_text(paragraph, &xml_general_ref(e.as_ref()));
                }
            }
            Ok(Event::CData(e)) => {
                if let Some(paragraph) = current.as_mut() {
                    docx_field_note_text(paragraph, &String::from_utf8_lossy(e.as_ref()));
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
    if !is_word {
        return;
    }

    if paragraph.simple.is_none() && parent == Some("p") && name == "fldSimple" {
        paragraph.simple = Some(DocxSimpleFieldState {
            instruction: docx_word_attr_ns(element, resolver, b"instr").unwrap_or_default(),
            depth: 1,
            ..DocxSimpleFieldState::default()
        });
        return;
    }

    if let Some(simple) = paragraph.simple.as_mut() {
        simple.depth += 1;
        if name == "t" && element_in_ns(resolver, element, DOCX_W_NS) {
            simple.in_t = true;
        }
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
        if simple.depth == 0 || name == "fldSimple" {
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

fn docx_field_code_base(code: &str) -> String {
    code.split_whitespace()
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase()
}

fn docx_field_location(part_uri: &str, block_index: usize) -> String {
    let prefix = if part_uri.ends_with("/document.xml") {
        "body".to_string()
    } else {
        let name = part_uri.rsplit('/').next().unwrap_or(part_uri);
        name.strip_suffix(".xml").unwrap_or(name).to_string()
    };
    format!("{prefix}:{block_index}")
}

struct DocxFieldLocation {
    part: String,
    block_index: usize,
    field_index: usize,
    has_field: bool,
}

struct DocxLocatedField {
    field_type: &'static str,
    instruction: String,
    cached_result: String,
    simple_range: Option<XmlNamedRange>,
    result_removals: Vec<XmlRange>,
    end_run_start: Option<usize>,
    end_run_prefix: String,
}

#[derive(Default)]
struct DocxComplexFieldBuilder {
    instruction: String,
    cached_result: String,
    result_removals: Vec<XmlRange>,
    end_run_start: Option<usize>,
    end_run_prefix: String,
    depth: usize,
    after_separator: bool,
}

pub(crate) fn docx_fields_insert(
    file: &str,
    location: &str,
    field_code: &str,
    result: &str,
    options: DocxParagraphMutationOptions<'_>,
) -> CliResult<Value> {
    let loc = parse_docx_field_location(location)?;
    if field_code.trim().is_empty() {
        return Err(CliError::invalid_args(
            "--field-code must be a non-empty instruction (e.g. PAGE)",
        ));
    }
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;
    let entries = zip_entry_names(file)?;
    ensure_docx_package_kind(file, &entries)?;
    let document_part = find_docx_document_part(file, &entries)?;
    let document_uri = format!("/{}", document_part.trim_start_matches('/'));
    let document_xml = zip_text(file, &document_part)?;
    let part_uri =
        docx_field_part_uri_for_location(file, &document_part, &document_uri, &document_xml, &loc)?;
    let part_entry = part_uri.trim_start_matches('/').to_string();
    let part_xml = if part_entry == document_part {
        document_xml
    } else {
        zip_text(file, &part_entry)?
    };
    let mutation = insert_docx_field_xml(
        &part_xml,
        &part_uri,
        &document_uri,
        loc.block_index,
        field_code,
        result,
    )?;
    let mut overrides = BTreeMap::new();
    overrides.insert(part_entry, mutation.xml);
    let output_path = docx_mutation_output_path_for_result(file, &options);
    write_docx_package_mutation_output(file, &overrides, options)?;

    let target = output_path.as_deref().unwrap_or(file);
    let mut object = Map::new();
    object.insert("file".to_string(), json!(file));
    object.insert("operation".to_string(), json!("inserted"));
    object.insert("partUri".to_string(), json!(part_uri));
    object.insert("blockIndex".to_string(), json!(mutation.block_index));
    object.insert("fieldIndex".to_string(), json!(mutation.field_index));
    object.insert("fieldType".to_string(), json!("simple"));
    object.insert("instruction".to_string(), json!(mutation.instruction));
    object.insert("cachedResult".to_string(), json!(result));
    object.insert("location".to_string(), json!(mutation.location));
    object.insert("paragraphText".to_string(), json!(mutation.paragraph_text));
    object.insert("knownCode".to_string(), json!(mutation.known_code));
    if !mutation.known_code {
        object.insert(
            "warning".to_string(),
            json!(format!(
                "field code {:?} is not a recognized instruction; inserted as-is (switches are not parsed)",
                mutation.instruction
            )),
        );
    }
    object.insert(
        "listCommand".to_string(),
        json!(docx_fields_list_command(target)),
    );
    object.insert(
        "validateCommand".to_string(),
        json!(docx_validate_strict_command(target)),
    );
    Ok(Value::Object(object))
}

pub(crate) fn docx_fields_set_result(
    file: &str,
    selector: &str,
    result: &str,
    expect_hash: &str,
    options: DocxParagraphMutationOptions<'_>,
) -> CliResult<Value> {
    let loc = parse_docx_field_location(selector)?;
    if !loc.has_field {
        return Err(CliError::invalid_args(format!(
            "invalid selector {selector:?}: a field index is required (e.g. body:1:0)"
        )));
    }
    validate_xlsx_mutation_output_flags(
        options.out,
        options.in_place,
        options.backup,
        options.dry_run,
    )?;
    let entries = zip_entry_names(file)?;
    ensure_docx_package_kind(file, &entries)?;
    let document_part = find_docx_document_part(file, &entries)?;
    let document_uri = format!("/{}", document_part.trim_start_matches('/'));
    let document_xml = zip_text(file, &document_part)?;
    let part_uri =
        docx_field_part_uri_for_location(file, &document_part, &document_uri, &document_xml, &loc)?;
    let part_entry = part_uri.trim_start_matches('/').to_string();
    let part_xml = if part_entry == document_part {
        document_xml
    } else {
        zip_text(file, &part_entry)?
    };
    let mutation = set_docx_field_result_xml(
        &part_xml,
        &part_uri,
        &document_uri,
        loc.block_index,
        loc.field_index,
        result,
        expect_hash,
    )?;
    let mut overrides = BTreeMap::new();
    overrides.insert(part_entry, mutation.xml);
    let output_path = docx_mutation_output_path_for_result(file, &options);
    write_docx_package_mutation_output(file, &overrides, options)?;

    let target = output_path.as_deref().unwrap_or(file);
    let mut object = Map::new();
    object.insert("file".to_string(), json!(file));
    object.insert("operation".to_string(), json!("set-result"));
    object.insert("partUri".to_string(), json!(part_uri));
    object.insert("blockIndex".to_string(), json!(mutation.block_index));
    object.insert("fieldIndex".to_string(), json!(loc.field_index));
    object.insert("fieldType".to_string(), json!(mutation.field_type));
    object.insert("instruction".to_string(), json!(mutation.instruction));
    object.insert(
        "previousResult".to_string(),
        json!(mutation.previous_result),
    );
    object.insert("cachedResult".to_string(), json!(result));
    object.insert("location".to_string(), json!(mutation.location));
    object.insert(
        "note".to_string(),
        json!("cachedResult is a cache; Word recomputes the live value on field recalculation"),
    );
    object.insert(
        "listCommand".to_string(),
        json!(docx_fields_list_command(target)),
    );
    object.insert(
        "validateCommand".to_string(),
        json!(docx_validate_strict_command(target)),
    );
    Ok(Value::Object(object))
}

fn parse_docx_field_location(value: &str) -> CliResult<DocxFieldLocation> {
    let parts = value.trim().split(':').collect::<Vec<_>>();
    if parts.len() < 2 || parts.len() > 3 {
        return Err(CliError::invalid_args(format!(
            "invalid location {value:?}: expected part:block[:field] (e.g. body:1 or header1:1:0)"
        )));
    }
    let part = parts[0].trim();
    if part.is_empty() {
        return Err(CliError::invalid_args(format!(
            "invalid location {value:?}: part segment is empty"
        )));
    }
    let block_index = parts[1].trim().parse::<usize>().map_err(|_| {
        CliError::invalid_args(format!(
            "invalid location {value:?}: block index must be a positive integer"
        ))
    })?;
    if block_index == 0 {
        return Err(CliError::invalid_args(format!(
            "invalid location {value:?}: block index must be a positive integer"
        )));
    }
    let (field_index, has_field) = if parts.len() == 3 {
        let field_index = parts[2].trim().parse::<usize>().map_err(|_| {
            CliError::invalid_args(format!(
                "invalid location {value:?}: field index must be a non-negative integer"
            ))
        })?;
        (field_index, true)
    } else {
        (0, false)
    };
    Ok(DocxFieldLocation {
        part: part.to_string(),
        block_index,
        field_index,
        has_field,
    })
}

fn docx_field_part_uri_for_location(
    file: &str,
    document_part: &str,
    document_uri: &str,
    document_xml: &str,
    loc: &DocxFieldLocation,
) -> CliResult<String> {
    if loc.part == "body" {
        return Ok(document_uri.to_string());
    }
    for part_uri in docx_header_footer_part_uris(file, document_part, document_uri, document_xml)? {
        let label = part_uri
            .rsplit('/')
            .next()
            .unwrap_or(part_uri.as_str())
            .trim_end_matches(".xml")
            .to_string();
        if label == loc.part {
            return Ok(part_uri);
        }
    }
    Err(CliError::target_not_found(format!(
        "part {:?} (use 'docx fields list' to discover locations)",
        loc.part
    )))
}

struct DocxFieldInsertMutation {
    xml: String,
    block_index: usize,
    field_index: usize,
    instruction: String,
    location: String,
    paragraph_text: String,
    known_code: bool,
}

struct DocxFieldSetResultMutation {
    xml: String,
    block_index: usize,
    field_type: String,
    instruction: String,
    previous_result: String,
    location: String,
}

fn insert_docx_field_xml(
    xml: &str,
    part_uri: &str,
    document_uri: &str,
    block_index: usize,
    field_code: &str,
    result: &str,
) -> CliResult<DocxFieldInsertMutation> {
    let paragraph = docx_field_target_paragraph_range(xml, part_uri, document_uri, block_index)?;
    let fragment = &xml[paragraph.start..paragraph.end];
    let existing_fields = docx_locate_fields_in_paragraph_fragment(fragment)?;
    let field_index = existing_fields.len();
    let instruction = normalize_docx_field_instruction(field_code)?;
    let updated_paragraph = append_docx_simple_field_to_paragraph(fragment, &instruction, result)?;
    let paragraph_text = docx_paragraph_fragment_text(&updated_paragraph);
    let mut updated_xml = String::with_capacity(xml.len() + updated_paragraph.len());
    updated_xml.push_str(&xml[..paragraph.start]);
    updated_xml.push_str(&updated_paragraph);
    updated_xml.push_str(&xml[paragraph.end..]);
    Ok(DocxFieldInsertMutation {
        xml: updated_xml,
        block_index,
        field_index,
        instruction: instruction.trim().to_string(),
        location: docx_field_location(part_uri, block_index),
        paragraph_text,
        known_code: is_known_docx_field_code(field_code),
    })
}

fn set_docx_field_result_xml(
    xml: &str,
    part_uri: &str,
    document_uri: &str,
    block_index: usize,
    field_index: usize,
    result: &str,
    expect_hash: &str,
) -> CliResult<DocxFieldSetResultMutation> {
    let paragraph = docx_field_target_paragraph_range(xml, part_uri, document_uri, block_index)?;
    let fragment = &xml[paragraph.start..paragraph.end];
    let fields = docx_locate_fields_in_paragraph_fragment(fragment)?;
    let field = fields
        .get(field_index)
        .ok_or_else(|| CliError::target_not_found("field"))?;
    if !expect_hash.is_empty() {
        let got = docx_field_content_hash(&field.instruction, &field.cached_result);
        if got != expect_hash {
            return Err(CliError::invalid_args(format!(
                "field hash mismatch: field expected {expect_hash} but found {got}"
            )));
        }
    }
    let updated_paragraph = if let Some(simple_range) = field.simple_range.as_ref() {
        replace_docx_simple_field_result(fragment, simple_range, result)?
    } else {
        replace_docx_complex_field_result(fragment, field, result)?
    };
    let mut updated_xml = String::with_capacity(xml.len() + updated_paragraph.len());
    updated_xml.push_str(&xml[..paragraph.start]);
    updated_xml.push_str(&updated_paragraph);
    updated_xml.push_str(&xml[paragraph.end..]);
    Ok(DocxFieldSetResultMutation {
        xml: updated_xml,
        block_index,
        field_type: field.field_type.to_string(),
        instruction: field.instruction.trim().to_string(),
        previous_result: field.cached_result.clone(),
        location: docx_field_location(part_uri, block_index),
    })
}

fn docx_field_target_paragraph_range(
    xml: &str,
    part_uri: &str,
    document_uri: &str,
    block_index: usize,
) -> CliResult<XmlRange> {
    if block_index == 0 {
        return Err(CliError::target_not_found("field target paragraph"));
    }
    if part_uri == document_uri || part_uri.ends_with("/document.xml") {
        let body_tag = docx_body_tag(xml)?;
        let blocks = docx_body_block_ranges(xml, &body_tag)?;
        let block = blocks
            .get(block_index - 1)
            .ok_or_else(|| CliError::target_not_found("field target paragraph"))?;
        if block.kind == "tbl" {
            return Err(CliError::invalid_args(format!(
                "field target is a table; table-nested fields are not addressable by the part:block:field selector (block {block_index}) (it is listed with editable=false by 'docx fields list'; editing table-nested fields is not yet supported)"
            )));
        }
        if block.kind != "p" {
            return Err(CliError::target_not_found("field target paragraph"));
        }
        return Ok(*block);
    }

    let root_tag = docx_header_footer_root_tag(xml, part_uri)?;
    let root_start = xml
        .find(&format!("<{root_tag}"))
        .ok_or_else(|| CliError::unexpected("invalid DOCX XML"))?;
    let root_open_end = xml[root_start..]
        .find('>')
        .map(|offset| root_start + offset)
        .ok_or_else(|| CliError::unexpected("invalid DOCX XML"))?;
    let root_self_closing = xml[root_start..=root_open_end].trim_end().ends_with("/>");
    if root_self_closing {
        return Err(CliError::target_not_found("field target paragraph"));
    }
    let root_close_start = xml
        .rfind(&format!("</{root_tag}>"))
        .ok_or_else(|| CliError::unexpected("invalid DOCX XML"))?;
    let paragraphs = xml_direct_child_ranges(xml, root_open_end + 1, root_close_start)?
        .into_iter()
        .filter(|child| child.kind == "p")
        .collect::<Vec<_>>();
    let paragraph = paragraphs
        .get(block_index - 1)
        .ok_or_else(|| CliError::target_not_found("field target paragraph"))?;
    Ok(XmlRange {
        start: paragraph.start,
        end: paragraph.end,
        kind: "p",
    })
}

fn docx_locate_fields_in_paragraph_fragment(fragment: &str) -> CliResult<Vec<DocxLocatedField>> {
    let (open_end, _tag_name, close_start, self_closing) = xml_fragment_bounds(fragment)?;
    if self_closing {
        return Ok(Vec::new());
    }
    let children = xml_direct_child_ranges(fragment, open_end + 1, close_start)?;
    let mut fields = Vec::new();
    let mut complex: Option<DocxComplexFieldBuilder> = None;

    for child in children {
        if child.kind == "fldSimple" && complex.is_none() {
            let field_fragment = &fragment[child.start..child.end];
            fields.push(DocxLocatedField {
                field_type: "simple",
                instruction: docx_first_word_attr(field_fragment, b"instr")
                    .unwrap_or_default()
                    .trim()
                    .to_string(),
                cached_result: docx_word_text_descendants(field_fragment, "t"),
                simple_range: Some(child),
                result_removals: Vec::new(),
                end_run_start: None,
                end_run_prefix: String::new(),
            });
            continue;
        }
        if child.kind != "r" {
            continue;
        }
        let run_fragment = &fragment[child.start..child.end];
        let (run_open_end, run_tag, run_close_start, run_self_closing) =
            xml_fragment_bounds(run_fragment)?;
        let run_prefix = xml_tag_prefix(&run_tag);
        if run_self_closing {
            continue;
        }
        let run_children =
            xml_direct_child_ranges(run_fragment, run_open_end + 1, run_close_start)?;
        let mut run_has_fld_char = false;
        let mut run_result_spans = Vec::new();
        let mut end_this_run = false;

        for run_child in run_children {
            match run_child.kind.as_str() {
                "fldChar" => {
                    run_has_fld_char = true;
                    let field_char_type = docx_first_word_attr(
                        &run_fragment[run_child.start..run_child.end],
                        b"fldCharType",
                    )
                    .unwrap_or_default();
                    match field_char_type.as_str() {
                        "begin" => {
                            if complex.is_none() {
                                complex = Some(DocxComplexFieldBuilder {
                                    depth: 1,
                                    after_separator: false,
                                    ..DocxComplexFieldBuilder::default()
                                });
                            } else if let Some(state) = complex.as_mut() {
                                state.depth += 1;
                            }
                        }
                        "separate" => {
                            if let Some(state) = complex.as_mut()
                                && state.depth == 1
                            {
                                state.after_separator = true;
                            }
                        }
                        "end" => {
                            if let Some(state) = complex.as_mut()
                                && state.depth > 0
                            {
                                state.depth -= 1;
                                if state.depth == 0 {
                                    state.end_run_start = Some(child.start);
                                    state.end_run_prefix = run_prefix.clone();
                                    end_this_run = true;
                                }
                            }
                        }
                        _ => {}
                    }
                }
                "instrText" => {
                    if let Some(state) = complex.as_mut()
                        && state.depth == 1
                        && !state.after_separator
                    {
                        state.instruction.push_str(&xml_fragment_text(
                            &run_fragment[run_child.start..run_child.end],
                        ));
                    }
                }
                "t" => {
                    if let Some(state) = complex.as_mut()
                        && state.depth == 1
                        && state.after_separator
                    {
                        state.cached_result.push_str(&xml_fragment_text(
                            &run_fragment[run_child.start..run_child.end],
                        ));
                        run_result_spans.push(XmlRange {
                            start: child.start + run_child.start,
                            end: child.start + run_child.end,
                            kind: "t",
                        });
                    }
                }
                _ => {}
            }
        }

        if !run_result_spans.is_empty()
            && let Some(state) = complex.as_mut()
        {
            if run_has_fld_char {
                state.result_removals.extend(run_result_spans);
            } else {
                state.result_removals.push(XmlRange {
                    start: child.start,
                    end: child.end,
                    kind: "r",
                });
            }
        }
        if end_this_run && let Some(state) = complex.take() {
            fields.push(DocxLocatedField {
                field_type: "complex",
                instruction: state.instruction.trim().to_string(),
                cached_result: state.cached_result,
                simple_range: None,
                result_removals: state.result_removals,
                end_run_start: state.end_run_start,
                end_run_prefix: state.end_run_prefix,
            });
        }
    }
    Ok(fields)
}

fn append_docx_simple_field_to_paragraph(
    fragment: &str,
    instruction: &str,
    result: &str,
) -> CliResult<String> {
    let (open_end, tag_name, close_start, self_closing) = xml_fragment_bounds(fragment)?;
    let start_tag = &fragment[..=open_end];
    let prefix = xml_tag_prefix(&tag_name);
    let mut out = String::with_capacity(fragment.len() + instruction.len() + result.len() + 80);
    out.push_str(&xml_open_tag_from_start(start_tag));
    if !self_closing {
        out.push_str(&fragment[open_end + 1..close_start]);
    }
    out.push_str(&render_docx_simple_field(&prefix, instruction, result));
    out.push_str("</");
    out.push_str(&tag_name);
    out.push('>');
    Ok(out)
}

fn replace_docx_simple_field_result(
    paragraph_fragment: &str,
    simple_range: &XmlNamedRange,
    result: &str,
) -> CliResult<String> {
    let field_fragment = &paragraph_fragment[simple_range.start..simple_range.end];
    let (open_end, tag_name, close_start, self_closing) = xml_fragment_bounds(field_fragment)?;
    let start_tag = &field_fragment[..=open_end];
    let prefix = xml_tag_prefix(&tag_name);
    let mut updated_field = String::with_capacity(field_fragment.len() + result.len() + 40);
    updated_field.push_str(&xml_open_tag_from_start(start_tag));
    if !self_closing {
        for child in xml_direct_child_ranges(field_fragment, open_end + 1, close_start)?
            .into_iter()
            .filter(|child| child.kind != "r")
        {
            updated_field.push_str(&field_fragment[child.start..child.end]);
        }
    }
    updated_field.push_str(&render_docx_field_result_run(&prefix, result));
    updated_field.push_str("</");
    updated_field.push_str(&tag_name);
    updated_field.push('>');

    let mut out = String::with_capacity(paragraph_fragment.len() + updated_field.len());
    out.push_str(&paragraph_fragment[..simple_range.start]);
    out.push_str(&updated_field);
    out.push_str(&paragraph_fragment[simple_range.end..]);
    Ok(out)
}

fn replace_docx_complex_field_result(
    paragraph_fragment: &str,
    field: &DocxLocatedField,
    result: &str,
) -> CliResult<String> {
    let end_run_start = field
        .end_run_start
        .ok_or_else(|| CliError::target_not_found("field"))?;
    let mut removals = field.result_removals.clone();
    removals.sort_by_key(|range| (range.start, range.end));
    removals.dedup_by_key(|range| (range.start, range.end));
    let removed_before_end = removals
        .iter()
        .filter(|range| range.start < end_run_start)
        .map(|range| range.end.saturating_sub(range.start))
        .sum::<usize>();
    let insert_at = end_run_start.saturating_sub(removed_before_end);
    let mut out = paragraph_fragment.to_string();
    for range in removals.into_iter().rev() {
        out.replace_range(range.start..range.end, "");
    }
    out.insert_str(
        insert_at,
        &render_docx_field_result_run(&field.end_run_prefix, result),
    );
    Ok(out)
}

fn render_docx_simple_field(prefix: &str, instruction: &str, result: &str) -> String {
    let fld = word_xml_tag(prefix, "fldSimple");
    let instr_attr = if prefix.is_empty() {
        "w:instr".to_string()
    } else {
        format!("{prefix}:instr")
    };
    let mut out = String::new();
    out.push('<');
    out.push_str(&fld);
    out.push(' ');
    out.push_str(&instr_attr);
    out.push_str("=\"");
    out.push_str(&xml_attr_escape(instruction));
    out.push_str("\">");
    out.push_str(&render_docx_field_result_run(prefix, result));
    out.push_str("</");
    out.push_str(&fld);
    out.push('>');
    out
}

fn render_docx_field_result_run(prefix: &str, result: &str) -> String {
    let r = word_xml_tag(prefix, "r");
    let t = word_xml_tag(prefix, "t");
    let mut out = String::new();
    out.push('<');
    out.push_str(&r);
    out.push('>');
    if result.is_empty() {
        out.push('<');
        out.push_str(&t);
        out.push_str("/>");
    } else {
        append_docx_text_children(&mut out, prefix, result);
    }
    out.push_str("</");
    out.push_str(&r);
    out.push('>');
    out
}

fn normalize_docx_field_instruction(code: &str) -> CliResult<String> {
    let trimmed = code.trim();
    if trimmed.is_empty() {
        return Err(CliError::invalid_args(
            "--field-code must be a non-empty instruction (e.g. PAGE)",
        ));
    }
    Ok(format!(" {trimmed} "))
}

fn is_known_docx_field_code(code: &str) -> bool {
    matches!(
        docx_field_code_base(code).as_str(),
        "PAGE" | "NUMPAGES" | "DATE" | "TIME" | "FILENAME" | "AUTHOR" | "SUBJECT" | "TITLE"
    )
}

fn docx_field_content_hash(instruction: &str, result: &str) -> String {
    let mut hash = Sha256::new();
    hash.update(instruction.trim().as_bytes());
    hash.update([0]);
    hash.update([0]);
    hash.update(result.as_bytes());
    format!("sha256:{:x}", hash.finalize())
}

fn docx_fields_list_command(file: &str) -> String {
    format!("ooxml --json docx fields list {file}")
}
