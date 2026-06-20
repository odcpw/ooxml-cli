mod parts;
mod selectors;
pub(crate) use parts::docx_header_footer_part_uris;
pub(crate) use selectors::normalize_docx_header_footer_show_type;
use selectors::{
    DocxHeaderFooterRefInfo, DocxHeaderFooterSelector,
    docx_header_footer_paragraph_primary_selector, docx_header_footer_paragraph_selectors,
    docx_header_footer_ref_info_from_parts, docx_header_footer_ref_json,
    parse_docx_header_footer_selector, resolve_docx_header_footer_selector,
};

use quick_xml::NsReader;
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::NamespaceResolver;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::{
    CliError, CliResult, DOCX_W_NS, DocxParagraphMutationOptions, InspectPackageKind,
    RelationshipEntry, XmlNamedRange, add_relationship_to_xml, allocate_relationship_id,
    append_docx_text_children, attr, attr_bound_ns, command_arg, content_type_for_part,
    decode_xml_text, detect_inspect_package_type, docx_body_content_bounds, docx_body_prefix,
    docx_body_tag, docx_mutation_output_path_for_result, docx_paragraph_fragment_text,
    docx_word_attr_ns, element_in_ns, ensure_content_type_override, ensure_docx_package_kind,
    ensure_docx_word_prefix, find_docx_document_part, first_direct_xml_child_by_kind, has_flag,
    json_i64, json_optional_string, local_name, package_type, parse_i64_flag, parse_string_flag,
    reject_unknown_flags, relationship_entries, relationship_target_from_source_to_target,
    relationships_part_for, resolve_relationship_target, validate_xlsx_mutation_output_flags,
    word_xml_tag, write_docx_package_mutation_output, xml_attr_escape, xml_direct_child_ranges,
    xml_fragment_bounds, xml_general_ref, xml_open_tag_from_start, xml_tag_prefix, zip_entry_names,
    zip_text,
};

pub(crate) fn docx_headers_footers_list(file: &str) -> CliResult<Value> {
    let (document_uri, sections) = docx_header_footer_listing(file)?;
    Ok(json!({
        "file": file,
        "documentPartUri": document_uri,
        "sections": sections,
    }))
}

fn docx_header_footer_listing(file: &str) -> CliResult<(String, Vec<Value>)> {
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
            "failed to list headers/footers: failed to read document part {document_uri}: {}",
            err.message
        ))
    })?;
    let rel_targets = relationship_entries(file, &relationships_part_for(&document_part))
        .unwrap_or_default()
        .into_iter()
        .filter(|rel| rel.target_mode != "External")
        .map(|rel| {
            (
                rel.id,
                resolve_relationship_target(&document_uri, &rel.target),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let sections = docx_header_footer_sections(file, &xml, &rel_targets)?;
    Ok((document_uri, sections))
}

#[derive(Default)]
struct DocxHeaderFooterSectionBuild {
    section_index: usize,
    headers: DocxHeaderFooterSetBuild,
    footers: DocxHeaderFooterSetBuild,
}

#[derive(Default)]
struct DocxHeaderFooterSetBuild {
    default: Option<Value>,
    first: Option<Value>,
    even: Option<Value>,
}

fn docx_header_footer_sections(
    file: &str,
    document_xml: &str,
    rel_targets: &BTreeMap<String, String>,
) -> CliResult<Vec<Value>> {
    let mut reader = NsReader::from_str(document_xml);
    let mut stack: Vec<String> = Vec::new();
    let mut sections = Vec::new();
    let mut current = None::<DocxHeaderFooterSectionBuild>;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let parent = stack.last().map(String::as_str);
                let grandparent = stack
                    .len()
                    .checked_sub(2)
                    .and_then(|index| stack.get(index))
                    .map(String::as_str);
                let is_word = element_in_ns(reader.resolver(), &e, DOCX_W_NS);
                if current.is_none()
                    && is_word
                    && name == "sectPr"
                    && (parent == Some("body") || parent == Some("pPr") && grandparent == Some("p"))
                {
                    current = Some(DocxHeaderFooterSectionBuild {
                        section_index: sections.len() + 1,
                        ..DocxHeaderFooterSectionBuild::default()
                    });
                } else if let Some(section) = current.as_mut()
                    && is_word
                    && matches!(name.as_str(), "headerReference" | "footerReference")
                {
                    docx_note_header_footer_ref(
                        file,
                        section,
                        &e,
                        reader.resolver(),
                        &name,
                        rel_targets,
                    );
                }
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let parent = stack.last().map(String::as_str);
                let grandparent = stack
                    .len()
                    .checked_sub(2)
                    .and_then(|index| stack.get(index))
                    .map(String::as_str);
                let is_word = element_in_ns(reader.resolver(), &e, DOCX_W_NS);
                if current.is_none()
                    && is_word
                    && name == "sectPr"
                    && (parent == Some("body") || parent == Some("pPr") && grandparent == Some("p"))
                {
                    let section = DocxHeaderFooterSectionBuild {
                        section_index: sections.len() + 1,
                        ..DocxHeaderFooterSectionBuild::default()
                    };
                    sections.push(docx_header_footer_section_json(section));
                } else if let Some(section) = current.as_mut()
                    && is_word
                    && matches!(name.as_str(), "headerReference" | "footerReference")
                {
                    docx_note_header_footer_ref(
                        file,
                        section,
                        &e,
                        reader.resolver(),
                        &name,
                        rel_targets,
                    );
                }
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if name == "sectPr"
                    && let Some(section) = current.take()
                {
                    sections.push(docx_header_footer_section_json(section));
                }
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }

    Ok(sections)
}

fn docx_note_header_footer_ref(
    file: &str,
    section: &mut DocxHeaderFooterSectionBuild,
    element: &BytesStart<'_>,
    resolver: &NamespaceResolver,
    name: &str,
    rel_targets: &BTreeMap<String, String>,
) {
    let kind = if name == "footerReference" {
        "footer"
    } else {
        "header"
    };
    let id = attr_bound_ns(
        element,
        resolver,
        b"http://schemas.openxmlformats.org/officeDocument/2006/relationships",
        b"id",
    )
    .unwrap_or_default();
    let ref_type = normalize_docx_header_footer_type(
        attr_bound_ns(element, resolver, DOCX_W_NS, b"type").unwrap_or_default(),
    );
    let part_uri = rel_targets.get(&id).cloned().unwrap_or_default();
    let content_type = if part_uri.is_empty() {
        String::new()
    } else {
        content_type_for_part(file, &part_uri).unwrap_or_default()
    };
    let value = docx_header_footer_ref_json(
        kind,
        &id,
        &ref_type,
        section.section_index,
        &part_uri,
        &content_type,
    );
    let set = if kind == "footer" {
        &mut section.footers
    } else {
        &mut section.headers
    };
    match ref_type.as_str() {
        "first" => set.first = Some(value),
        "even" => set.even = Some(value),
        _ => set.default = Some(value),
    }
}

fn normalize_docx_header_footer_type(value: String) -> String {
    match value.as_str() {
        "first" | "even" => value,
        _ => "default".to_string(),
    }
}

fn docx_header_footer_section_json(section: DocxHeaderFooterSectionBuild) -> Value {
    json!({
        "sectionIndex": section.section_index,
        "headers": docx_header_footer_set_json(section.headers),
        "footers": docx_header_footer_set_json(section.footers),
    })
}

fn docx_header_footer_set_json(set: DocxHeaderFooterSetBuild) -> Value {
    json!({
        "default": set.default.unwrap_or(Value::Null),
        "first": set.first.unwrap_or(Value::Null),
        "even": set.even.unwrap_or(Value::Null),
    })
}

pub(crate) fn docx_headers_footers_show(
    file: &str,
    kind: &str,
    rest: &[String],
) -> CliResult<Value> {
    reject_unknown_flags(rest, &["--id", "--type", "--section", "--selector"], &[])?;
    let id = parse_string_flag(rest, "--id")?.unwrap_or_default();
    let ref_type = parse_string_flag(rest, "--type")?.unwrap_or_else(|| "default".to_string());
    let ref_type = normalize_docx_header_footer_show_type(&ref_type)?;
    let section = parse_i64_flag(rest, "--section")?.unwrap_or(0);
    if section < 0 {
        return Err(CliError::invalid_args(
            "--section must be >= 0 (0 means the last section)",
        ));
    }
    let selector = parse_string_flag(rest, "--selector")?;
    if selector.is_some()
        && (has_flag(rest, "--id") || has_flag(rest, "--type") || has_flag(rest, "--section"))
    {
        return Err(CliError::invalid_args(
            "cannot specify --selector with --id, --type, or --section",
        ));
    }

    let (_document_uri, sections) = docx_header_footer_listing(file)?;
    let target = if let Some(selector) = selector {
        let parsed = parse_docx_header_footer_selector(kind, &selector)?;
        resolve_docx_header_footer_selector(&sections, kind, &parsed)
    } else if !id.is_empty() {
        resolve_docx_header_footer_selector(
            &sections,
            kind,
            &DocxHeaderFooterSelector {
                kind: kind.to_string(),
                id,
                ref_type,
                section,
                ..DocxHeaderFooterSelector::default()
            },
        )
    } else {
        resolve_docx_header_footer_selector(
            &sections,
            kind,
            &DocxHeaderFooterSelector {
                kind: kind.to_string(),
                ref_type,
                section,
                ..DocxHeaderFooterSelector::default()
            },
        )
    }
    .ok_or_else(|| CliError::target_not_found(format!("target not found: {kind}")))?;

    if target.part_uri.is_empty() {
        return Err(CliError::invalid_args(format!(
            "{kind} reference {:?} does not resolve to a part",
            target.id
        )));
    }
    let paragraphs = docx_header_footer_paragraphs(file, &target)?;
    Ok(json!({
        "file": file,
        "kind": target.kind,
        "partUri": target.part_uri,
        "id": target.id,
        "type": target.ref_type,
        "section": target.section,
        "primarySelector": target.primary_selector,
        "selectors": target.selectors,
        "paragraphs": paragraphs,
    }))
}

pub(crate) fn docx_header_footer_kind(group: &str) -> &'static str {
    if group == "footers" {
        "footer"
    } else {
        "header"
    }
}

fn docx_header_footer_paragraphs(
    file: &str,
    reference: &DocxHeaderFooterRefInfo,
) -> CliResult<Vec<Value>> {
    let xml = zip_text(file, reference.part_uri.trim_start_matches('/')).map_err(|err| {
        CliError::unexpected(format!(
            "failed to read header/footer part {}: {}",
            reference.part_uri, err.message
        ))
    })?;
    let mut reader = NsReader::from_str(&xml);
    let mut stack = Vec::<String>::new();
    let mut paragraphs = Vec::new();
    let mut current = None::<DocxHeaderFooterParagraphBuild>;
    let mut in_t = false;
    let mut skip_text_depth = 0usize;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let is_word = element_in_ns(reader.resolver(), &e, DOCX_W_NS);
                if stack.len() == 1 && is_word && name == "p" {
                    current = Some(DocxHeaderFooterParagraphBuild::default());
                }
                docx_note_header_footer_paragraph_start(
                    &mut current,
                    &e,
                    reader.resolver(),
                    &stack,
                    is_word,
                    skip_text_depth,
                );
                if is_word && name == "t" {
                    in_t = true;
                }
                if is_word && matches!(name.as_str(), "delText" | "instrText") {
                    skip_text_depth += 1;
                }
                stack.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let is_word = element_in_ns(reader.resolver(), &e, DOCX_W_NS);
                if stack.len() == 1 && is_word && name == "p" {
                    let paragraph = DocxHeaderFooterParagraphBuild::default();
                    paragraphs.push(docx_header_footer_paragraph_json(
                        paragraphs.len() + 1,
                        paragraph,
                        reference,
                    ));
                } else {
                    docx_note_header_footer_paragraph_start(
                        &mut current,
                        &e,
                        reader.resolver(),
                        &stack,
                        is_word,
                        skip_text_depth,
                    );
                }
            }
            Ok(Event::Text(e)) if in_t && skip_text_depth == 0 => {
                if let Some(paragraph) = current.as_mut() {
                    paragraph.text.push_str(&decode_xml_text(e.as_ref()));
                }
            }
            Ok(Event::GeneralRef(e)) if in_t && skip_text_depth == 0 => {
                if let Some(paragraph) = current.as_mut() {
                    paragraph.text.push_str(&xml_general_ref(e.as_ref()));
                }
            }
            Ok(Event::CData(e)) if in_t && skip_text_depth == 0 => {
                if let Some(paragraph) = current.as_mut() {
                    paragraph
                        .text
                        .push_str(&String::from_utf8_lossy(e.as_ref()));
                }
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if name == "t" {
                    in_t = false;
                } else if matches!(name.as_str(), "delText" | "instrText") {
                    skip_text_depth = skip_text_depth.saturating_sub(1);
                } else if name == "p"
                    && let Some(paragraph) = current.take()
                {
                    paragraphs.push(docx_header_footer_paragraph_json(
                        paragraphs.len() + 1,
                        paragraph,
                        reference,
                    ));
                }
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }

    Ok(paragraphs)
}

#[derive(Default)]
struct DocxHeaderFooterParagraphBuild {
    style: String,
    text: String,
}

fn docx_note_header_footer_paragraph_start(
    current: &mut Option<DocxHeaderFooterParagraphBuild>,
    element: &BytesStart<'_>,
    resolver: &NamespaceResolver,
    stack: &[String],
    is_word: bool,
    skip_text_depth: usize,
) {
    let Some(paragraph) = current.as_mut() else {
        return;
    };
    let qualified_name = element.name();
    let name = local_name(qualified_name.as_ref());
    if is_word
        && name == "pStyle"
        && stack.last().is_some_and(|parent| parent == "pPr")
        && let Some(style) = docx_word_attr_ns(element, resolver, b"val")
    {
        paragraph.style = style;
        return;
    }
    if is_word && skip_text_depth == 0 {
        match name {
            "tab" => paragraph.text.push('\t'),
            "br" | "cr" => paragraph.text.push('\n'),
            "noBreakHyphen" => paragraph.text.push('-'),
            _ => {}
        }
    }
}

fn docx_header_footer_paragraph_json(
    index: usize,
    paragraph: DocxHeaderFooterParagraphBuild,
    reference: &DocxHeaderFooterRefInfo,
) -> Value {
    let primary_selector = if reference.primary_selector.is_empty() {
        String::new()
    } else {
        format!("{}/p:{index}", reference.primary_selector)
    };
    let mut selectors = Vec::new();
    for selector in &reference.selectors {
        selectors.push(format!("{selector}/p:{index}"));
        selectors.push(format!("{selector}/paragraph:{index}"));
    }
    json!({
        "index": index,
        "primarySelector": primary_selector,
        "selectors": selectors,
        "style": paragraph.style,
        "text": paragraph.text,
    })
}

pub(crate) struct DocxHeaderFooterSetTextOptions<'a> {
    pub(crate) id: &'a str,
    pub(crate) ref_type: &'a str,
    pub(crate) section: i64,
    pub(crate) index: i64,
    pub(crate) selector: Option<&'a str>,
    pub(crate) selector_given: bool,
    pub(crate) index_given: bool,
    pub(crate) text: &'a str,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) in_place: bool,
    pub(crate) no_validate: bool,
}

struct DocxHeaderFooterEnsureResult {
    document_xml: String,
    rels_part: Option<String>,
    rels_xml: Option<String>,
    content_types_xml: Option<String>,
    part_xml: Option<String>,
    reference: DocxHeaderFooterRefInfo,
    created_part: bool,
    created_ref: bool,
}

struct DocxHeaderFooterEnsureContext<'a> {
    file: &'a str,
    entries: &'a [String],
    document_part: &'a str,
    document_uri: &'a str,
    document_xml: &'a str,
}

#[derive(Clone, Copy)]
struct DocxSectionRange {
    index: i64,
    start: usize,
    end: usize,
}

pub(crate) fn docx_headers_footers_set_text(
    file: &str,
    kind: &str,
    mut options: DocxHeaderFooterSetTextOptions<'_>,
) -> CliResult<Value> {
    if !Path::new(file).exists() {
        return Err(CliError::file_not_found(format!("file not found: {file}")));
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

    let mut created_part = false;
    let mut created_ref = false;
    let mut document_override = None::<String>;
    let mut rels_override = None::<(String, String)>;
    let mut content_types_override = None::<String>;
    let mut part_xml_override = None::<String>;

    let reference = if options.selector_given {
        let selector = options.selector.unwrap_or_default();
        let parsed = parse_docx_header_footer_selector(kind, selector)?;
        if parsed.paragraph_index > 0 {
            if options.index_given && options.index != parsed.paragraph_index {
                return Err(CliError::invalid_args(
                    "--index conflicts with the paragraph index embedded in --selector",
                ));
            }
            options.index = parsed.paragraph_index;
        }
        if !parsed.id.is_empty() || !parsed.part_uri.is_empty() {
            let (_document_uri, sections) = docx_header_footer_listing(file)?;
            resolve_docx_header_footer_selector(&sections, kind, &parsed).ok_or_else(|| {
                CliError::target_not_found(format!("{kind} not found: {selector}"))
            })?
        } else {
            let ensured = ensure_docx_header_footer(
                DocxHeaderFooterEnsureContext {
                    file,
                    entries: &entries,
                    document_part: &document_part,
                    document_uri: &document_uri,
                    document_xml: &document_xml,
                },
                kind,
                &parsed.ref_type,
                parsed.section,
            )?;
            created_part = ensured.created_part;
            created_ref = ensured.created_ref;
            document_override = Some(ensured.document_xml);
            if let (Some(part), Some(xml)) = (ensured.rels_part, ensured.rels_xml) {
                rels_override = Some((part, xml));
            }
            content_types_override = ensured.content_types_xml;
            part_xml_override = ensured.part_xml;
            ensured.reference
        }
    } else if !options.id.is_empty() {
        let (_document_uri, sections) = docx_header_footer_listing(file)?;
        resolve_docx_header_footer_selector(
            &sections,
            kind,
            &DocxHeaderFooterSelector {
                kind: kind.to_string(),
                id: options.id.to_string(),
                ref_type: options.ref_type.to_string(),
                section: options.section,
                ..DocxHeaderFooterSelector::default()
            },
        )
        .ok_or_else(|| CliError::target_not_found(format!("{kind} not found: id:{}", options.id)))?
    } else {
        let ensured = ensure_docx_header_footer(
            DocxHeaderFooterEnsureContext {
                file,
                entries: &entries,
                document_part: &document_part,
                document_uri: &document_uri,
                document_xml: &document_xml,
            },
            kind,
            options.ref_type,
            options.section,
        )?;
        created_part = ensured.created_part;
        created_ref = ensured.created_ref;
        document_override = Some(ensured.document_xml);
        if let (Some(part), Some(xml)) = (ensured.rels_part, ensured.rels_xml) {
            rels_override = Some((part, xml));
        }
        content_types_override = ensured.content_types_xml;
        part_xml_override = ensured.part_xml;
        ensured.reference
    };

    if reference.part_uri.is_empty() {
        return Err(CliError::invalid_args(format!(
            "{kind} reference {:?} does not resolve to a part",
            reference.id
        )));
    }

    let part_name = reference.part_uri.trim_start_matches('/').to_string();
    let part_xml = if let Some(xml) = part_xml_override {
        xml
    } else {
        zip_text(file, &part_name).map_err(|_| {
            CliError::target_not_found(format!("{kind} part not found: {}", reference.part_uri))
        })?
    };
    let mutation = set_docx_header_footer_text_xml(
        &part_xml,
        &reference.part_uri,
        options.index,
        options.text,
    )?;

    let mut overrides = BTreeMap::new();
    if let Some(xml) = document_override.filter(|xml| xml != &document_xml) {
        overrides.insert(document_part.clone(), xml);
    }
    if let Some((part, xml)) = rels_override {
        overrides.insert(part, xml);
    }
    if let Some(xml) = content_types_override {
        overrides.insert("[Content_Types].xml".to_string(), xml);
    }
    overrides.insert(part_name, mutation.xml);

    let output_path = docx_mutation_output_path_for_result(
        file,
        &DocxParagraphMutationOptions {
            text: None,
            text_file: None,
            style: "",
            out: options.out,
            backup: options.backup,
            dry_run: options.dry_run,
            in_place: options.in_place,
            no_validate: options.no_validate,
        },
    );
    write_docx_package_mutation_output(
        file,
        &overrides,
        DocxParagraphMutationOptions {
            text: None,
            text_file: None,
            style: "",
            out: options.out,
            backup: options.backup,
            dry_run: options.dry_run,
            in_place: options.in_place,
            no_validate: options.no_validate,
        },
    )?;

    let paragraph_primary =
        docx_header_footer_paragraph_primary_selector(&reference.primary_selector, mutation.index);
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    if let Some(output) = output_path.as_deref() {
        result.insert("output".to_string(), json!(output));
    }
    result.insert("dryRun".to_string(), json!(options.dry_run));
    result.insert("kind".to_string(), json!(reference.kind));
    result.insert("partUri".to_string(), json!(reference.part_uri));
    result.insert("id".to_string(), json!(reference.id));
    result.insert("type".to_string(), json!(reference.ref_type));
    result.insert("section".to_string(), json!(reference.section));
    result.insert(
        "primarySelector".to_string(),
        json!(reference.primary_selector),
    );
    result.insert("selectors".to_string(), json!(reference.selectors));
    result.insert("paragraphIndex".to_string(), json!(mutation.index));
    result.insert(
        "paragraphPrimarySelector".to_string(),
        json!(paragraph_primary),
    );
    result.insert(
        "paragraphSelectors".to_string(),
        json!(docx_header_footer_paragraph_selectors(
            &reference.selectors,
            mutation.index
        )),
    );
    result.insert("previousText".to_string(), json!(mutation.previous_text));
    result.insert("text".to_string(), json!(options.text));
    result.insert("createdPart".to_string(), json!(created_part));
    result.insert("createdRef".to_string(), json!(created_ref));
    add_docx_header_footer_readback_commands(
        &mut result,
        output_path.as_deref(),
        &reference.kind,
        &reference.primary_selector,
    );
    Ok(Value::Object(result))
}

fn ensure_docx_header_footer(
    ctx: DocxHeaderFooterEnsureContext<'_>,
    kind: &str,
    ref_type: &str,
    section_index: i64,
) -> CliResult<DocxHeaderFooterEnsureResult> {
    if let Some(section) = select_docx_section_range(ctx.document_xml, section_index)?
        && let Some(id) = docx_header_footer_reference_id(
            &ctx.document_xml[section.start..section.end],
            kind,
            ref_type,
        )
    {
        let rels = relationship_entries(ctx.file, &relationships_part_for(ctx.document_part))
            .unwrap_or_default();
        let part_uri = rels
            .iter()
            .find(|rel| rel.id == id)
            .map(|rel| resolve_relationship_target(ctx.document_uri, &rel.target))
            .unwrap_or_default();
        return Ok(DocxHeaderFooterEnsureResult {
            document_xml: ctx.document_xml.to_string(),
            rels_part: None,
            rels_xml: None,
            content_types_xml: None,
            part_xml: None,
            reference: docx_header_footer_ref_info_from_parts(
                kind,
                &id,
                ref_type,
                section.index,
                &part_uri,
            ),
            created_part: false,
            created_ref: false,
        });
    }

    let mut working = ctx.document_xml.to_string();
    if docx_body_prefix(&docx_body_tag(&working)?).is_empty() {
        working = ensure_docx_word_prefix(&working)?;
    }
    working = ensure_docx_relationship_namespace(&working)?;
    let (mut working, section) = select_or_create_docx_section_range(working, section_index)?;

    let rels_part = relationships_part_for(ctx.document_part);
    let rels = relationship_entries(ctx.file, &rels_part).unwrap_or_default();
    let referenced = docx_referenced_header_footer_rel_ids(&working);
    let mut created_part = false;
    let mut rels_xml = None::<String>;
    let mut content_types_xml = None::<String>;
    let mut part_xml = None::<String>;
    let (id, part_uri) = if let Some((id, part_uri)) =
        unreferenced_docx_header_footer_part(&rels, &referenced, ctx.document_uri, kind)
    {
        (id, part_uri)
    } else {
        let part_uri = allocate_docx_header_footer_part_uri(ctx.entries, kind);
        let id = allocate_relationship_id(&rels);
        let target = relationship_target_from_source_to_target(ctx.document_uri, &part_uri);
        let rel_xml = add_relationship_to_xml(
            zip_text(ctx.file, &rels_part).unwrap_or_else(|_| {
                r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"></Relationships>"#
                    .to_string()
            }),
            &id,
            docx_header_footer_relationship_type(kind),
            &target,
        );
        let content_xml = ensure_content_type_override(
            zip_text(ctx.file, "[Content_Types].xml")?,
            &part_uri,
            docx_header_footer_content_type(kind),
        );
        created_part = true;
        rels_xml = Some(rel_xml);
        content_types_xml = Some(content_xml);
        part_xml = Some(docx_header_footer_template(kind));
        (id, part_uri)
    };

    working = insert_docx_header_footer_reference(&working, section, kind, ref_type, &id)?;

    Ok(DocxHeaderFooterEnsureResult {
        document_xml: working,
        rels_part: rels_xml.as_ref().map(|_| rels_part),
        rels_xml,
        content_types_xml,
        part_xml,
        reference: docx_header_footer_ref_info_from_parts(
            kind,
            &id,
            ref_type,
            section.index,
            &part_uri,
        ),
        created_part,
        created_ref: true,
    })
}

fn select_docx_section_range(xml: &str, section_index: i64) -> CliResult<Option<DocxSectionRange>> {
    let sections = docx_section_ranges(xml)?;
    if sections.is_empty() {
        return Ok(None);
    }
    let selected = if section_index <= 0 {
        *sections.last().expect("nonempty sections")
    } else {
        *sections.get(section_index as usize - 1).ok_or_else(|| {
            CliError::unexpected(format!(
                "failed to mutate header: section {section_index} out of range (document has {} sections)",
                sections.len()
            ))
        })?
    };
    Ok(Some(selected))
}

fn select_or_create_docx_section_range(
    mut xml: String,
    section_index: i64,
) -> CliResult<(String, DocxSectionRange)> {
    if let Some(section) = select_docx_section_range(&xml, section_index)? {
        return Ok((xml, section));
    }
    let body_tag = docx_body_tag(&xml)?;
    let prefix = docx_body_prefix(&body_tag);
    let body_close = xml
        .rfind(&format!("</{body_tag}>"))
        .ok_or_else(|| CliError::unexpected("document body element not found"))?;
    let sect_pr = format!("<{}/>", word_xml_tag(&prefix, "sectPr"));
    xml.insert_str(body_close, &sect_pr);
    Ok((
        xml,
        DocxSectionRange {
            index: 1,
            start: body_close,
            end: body_close + sect_pr.len(),
        },
    ))
}

fn docx_section_ranges(xml: &str) -> CliResult<Vec<DocxSectionRange>> {
    let body_tag = docx_body_tag(xml)?;
    let (content_start, content_end) = docx_body_content_bounds(xml, &body_tag)?;
    let mut sections = Vec::new();
    for child in xml_direct_child_ranges(xml, content_start, content_end)? {
        if child.kind == "sectPr" {
            sections.push(DocxSectionRange {
                index: sections.len() as i64 + 1,
                start: child.start,
                end: child.end,
            });
            continue;
        }
        if child.kind != "p" {
            continue;
        }
        let Some(p_pr) = direct_child_range_by_kind(xml, child, "pPr")? else {
            continue;
        };
        let Some(sect_pr) = direct_child_range_by_kind(xml, p_pr, "sectPr")? else {
            continue;
        };
        sections.push(DocxSectionRange {
            index: sections.len() as i64 + 1,
            start: sect_pr.start,
            end: sect_pr.end,
        });
    }
    Ok(sections)
}

fn direct_child_range_by_kind(
    xml: &str,
    range: XmlNamedRange,
    wanted: &str,
) -> CliResult<Option<XmlNamedRange>> {
    let fragment = &xml[range.start..range.end];
    let (open_end, _tag_name, close_start, self_closing) = xml_fragment_bounds(fragment)?;
    if self_closing {
        return Ok(None);
    }
    Ok(
        xml_direct_child_ranges(xml, range.start + open_end + 1, range.start + close_start)?
            .into_iter()
            .find(|child| child.kind == wanted),
    )
}

fn docx_header_footer_reference_id(fragment: &str, kind: &str, ref_type: &str) -> Option<String> {
    let wanted = format!("{kind}Reference");
    let mut reader = NsReader::from_str(fragment);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == wanted =>
            {
                let actual_type =
                    normalize_docx_header_footer_type(attr(&e, "type").unwrap_or_default());
                if actual_type == ref_type {
                    return attr(&e, "id");
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    None
}

fn docx_referenced_header_footer_rel_ids(xml: &str) -> BTreeSet<String> {
    let mut reader = NsReader::from_str(xml);
    let mut ids = BTreeSet::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if matches!(
                    local_name(e.name().as_ref()),
                    "headerReference" | "footerReference"
                ) =>
            {
                if let Some(id) = attr(&e, "id") {
                    ids.insert(id);
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    ids
}

fn unreferenced_docx_header_footer_part(
    rels: &[RelationshipEntry],
    referenced: &BTreeSet<String>,
    document_uri: &str,
    kind: &str,
) -> Option<(String, String)> {
    let rel_type = docx_header_footer_relationship_type(kind);
    rels.iter()
        .find(|rel| {
            rel.rel_type == rel_type
                && rel.target_mode != "External"
                && !referenced.contains(&rel.id)
        })
        .map(|rel| {
            (
                rel.id.clone(),
                resolve_relationship_target(document_uri, &rel.target),
            )
        })
}

fn allocate_docx_header_footer_part_uri(entries: &[String], kind: &str) -> String {
    let prefix = format!("word/{kind}");
    let mut used = BTreeSet::new();
    for entry in entries {
        let normalized = entry.trim_start_matches('/');
        if !normalized.starts_with(&prefix) || !normalized.ends_with(".xml") {
            continue;
        }
        let number = normalized
            .trim_start_matches(&prefix)
            .trim_end_matches(".xml")
            .parse::<u32>();
        if let Ok(number) = number {
            used.insert(number);
        }
    }
    let mut next = 1;
    while used.contains(&next) {
        next += 1;
    }
    format!("/word/{kind}{next}.xml")
}

fn insert_docx_header_footer_reference(
    xml: &str,
    section: DocxSectionRange,
    kind: &str,
    ref_type: &str,
    id: &str,
) -> CliResult<String> {
    let fragment = &xml[section.start..section.end];
    let (open_end, tag_name, close_start, self_closing) = xml_fragment_bounds(fragment)?;
    let prefix = xml_tag_prefix(&tag_name);
    let ref_tag = word_xml_tag(&prefix, &format!("{kind}Reference"));
    let type_attr = if prefix.is_empty() {
        "w:type".to_string()
    } else {
        format!("{prefix}:type")
    };
    let reference = format!(
        r#"<{ref_tag} {type_attr}="{}" r:id="{}"/>"#,
        xml_attr_escape(ref_type),
        xml_attr_escape(id)
    );
    let mut updated = xml_open_tag_from_start(&fragment[..=open_end]);
    if self_closing {
        updated.push_str(&reference);
    } else {
        let children = xml_direct_child_ranges(fragment, open_end + 1, close_start)?;
        let insert_at = children
            .iter()
            .find(|child| child.kind != "headerReference" && child.kind != "footerReference")
            .map(|child| child.start)
            .unwrap_or(close_start);
        updated.push_str(&fragment[open_end + 1..insert_at]);
        updated.push_str(&reference);
        updated.push_str(&fragment[insert_at..close_start]);
    }
    updated.push_str("</");
    updated.push_str(&tag_name);
    updated.push('>');

    let mut out = String::with_capacity(xml.len() + updated.len());
    out.push_str(&xml[..section.start]);
    out.push_str(&updated);
    out.push_str(&xml[section.end..]);
    Ok(out)
}

struct DocxHeaderFooterTextMutation {
    xml: String,
    index: i64,
    previous_text: String,
}

fn set_docx_header_footer_text_xml(
    xml: &str,
    part_uri: &str,
    index: i64,
    text: &str,
) -> CliResult<DocxHeaderFooterTextMutation> {
    let root_tag = docx_header_footer_root_tag(xml, part_uri)?;
    let root_start = xml.find(&format!("<{root_tag}")).ok_or_else(|| {
        CliError::unexpected(format!("part {part_uri} is not a header or footer"))
    })?;
    let root_open_end = xml[root_start..]
        .find('>')
        .map(|offset| root_start + offset)
        .ok_or_else(|| CliError::unexpected("invalid DOCX XML"))?;
    let root_self_closing = xml[root_start..=root_open_end].trim_end().ends_with("/>");
    let root_close_start = if root_self_closing {
        root_open_end + 1
    } else {
        xml.rfind(&format!("</{root_tag}>"))
            .ok_or_else(|| CliError::unexpected("invalid DOCX XML"))?
    };
    let paragraphs: Vec<XmlNamedRange> = if root_self_closing {
        Vec::new()
    } else {
        xml_direct_child_ranges(xml, root_open_end + 1, root_close_start)?
            .into_iter()
            .filter(|child| child.kind == "p")
            .collect()
    };
    let paragraph = paragraphs.get(index as usize - 1).ok_or_else(|| {
        CliError::target_not_found(format!("target not found: header/footer paragraph {index}"))
    })?;
    let fragment = &xml[paragraph.start..paragraph.end];
    let previous_text = docx_paragraph_fragment_text(fragment);
    let updated_paragraph = replace_docx_header_footer_paragraph_fragment(fragment, text)?;
    let mut out = String::with_capacity(xml.len() + updated_paragraph.len());
    out.push_str(&xml[..paragraph.start]);
    out.push_str(&updated_paragraph);
    out.push_str(&xml[paragraph.end..]);
    Ok(DocxHeaderFooterTextMutation {
        xml: out,
        index,
        previous_text,
    })
}

fn replace_docx_header_footer_paragraph_fragment(fragment: &str, text: &str) -> CliResult<String> {
    let (open_end, tag_name, close_start, self_closing) = xml_fragment_bounds(fragment)?;
    let start_tag = &fragment[..=open_end];
    let prefix = xml_tag_prefix(&tag_name);
    let mut paragraph_properties = String::new();
    let mut run_properties = String::new();
    if !self_closing {
        for child in xml_direct_child_ranges(fragment, open_end + 1, close_start)? {
            match child.kind.as_str() {
                "pPr" if paragraph_properties.is_empty() => {
                    paragraph_properties.push_str(&fragment[child.start..child.end]);
                }
                "r" if run_properties.is_empty() => {
                    if let Some(r_pr) =
                        first_direct_xml_child_by_kind(&fragment[child.start..child.end], "rPr")?
                    {
                        run_properties.push_str(&r_pr);
                    }
                }
                _ => {}
            }
        }
    }
    let mut out = xml_open_tag_from_start(start_tag);
    out.push_str(&paragraph_properties);
    let r = word_xml_tag(&prefix, "r");
    out.push('<');
    out.push_str(&r);
    out.push('>');
    out.push_str(&run_properties);
    append_docx_text_children(&mut out, &prefix, text);
    out.push_str("</");
    out.push_str(&r);
    out.push('>');
    out.push_str("</");
    out.push_str(&tag_name);
    out.push('>');
    Ok(out)
}

pub(crate) fn docx_header_footer_root_tag(xml: &str, part_uri: &str) -> CliResult<String> {
    let mut reader = NsReader::from_str(xml);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if element_in_ns(reader.resolver(), &e, DOCX_W_NS)
                    && matches!(name.as_str(), "hdr" | "ftr")
                {
                    return Ok(String::from_utf8_lossy(e.name().as_ref()).to_string());
                }
                return Err(CliError::unexpected(format!(
                    "part {part_uri} is not a header or footer"
                )));
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Err(CliError::unexpected(format!(
        "part {part_uri} is not a header or footer"
    )))
}

fn add_docx_header_footer_readback_commands(
    result: &mut Map<String, Value>,
    output_path: Option<&str>,
    kind: &str,
    selector: &str,
) {
    let target = output_path.unwrap_or("<out.docx>");
    let validate = format!("ooxml validate --strict {target}");
    let show = docx_header_footer_show_command(target, kind, selector);
    let list = docx_header_footer_list_command(target, kind);
    if output_path.is_some() {
        result.insert("validateCommand".to_string(), json!(validate));
        result.insert("showCommand".to_string(), json!(show));
        result.insert("listCommand".to_string(), json!(list));
    } else {
        result.insert("validateCommandTemplate".to_string(), json!(validate));
        result.insert("showCommandTemplate".to_string(), json!(show));
        result.insert("listCommandTemplate".to_string(), json!(list));
    }
}

fn docx_header_footer_show_command(file: &str, kind: &str, selector: &str) -> String {
    let group = if kind == "footer" {
        "footers"
    } else {
        "headers"
    };
    let mut command = format!("ooxml --json docx {group} show {}", command_arg(file));
    if !selector.trim().is_empty() {
        command.push_str(" --selector ");
        command.push_str(&command_arg(selector));
    }
    command
}

fn docx_header_footer_list_command(file: &str, kind: &str) -> String {
    let group = if kind == "footer" {
        "footers"
    } else {
        "headers"
    };
    format!("ooxml --json docx {group} list {}", command_arg(file))
}

fn ensure_docx_relationship_namespace(xml: &str) -> CliResult<String> {
    if xml
        .contains("xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"")
    {
        return Ok(xml.to_string());
    }
    let document_start = xml
        .find("<w:document")
        .or_else(|| xml.find("<document"))
        .ok_or_else(|| CliError::unexpected("document root element not found"))?;
    let start_end = xml[document_start..]
        .find('>')
        .map(|offset| document_start + offset)
        .ok_or_else(|| CliError::unexpected("document root element not found"))?;
    let mut out = String::with_capacity(xml.len() + 88);
    out.push_str(&xml[..start_end]);
    out.push_str(
        " xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"",
    );
    out.push_str(&xml[start_end..]);
    Ok(out)
}

fn docx_header_footer_template(kind: &str) -> String {
    let tag = if kind == "footer" { "w:ftr" } else { "w:hdr" };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><{tag} xmlns:w="{}" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:p/></{tag}>"#,
        String::from_utf8_lossy(DOCX_W_NS)
    )
}

fn docx_header_footer_content_type(kind: &str) -> &'static str {
    if kind == "footer" {
        "application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"
    } else {
        "application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"
    }
}

fn docx_header_footer_relationship_type(kind: &str) -> &'static str {
    if kind == "footer" {
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer"
    } else {
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/header"
    }
}

pub(crate) fn docx_header_footer_show_json_args(args: &Value) -> CliResult<Vec<String>> {
    let mut rest = Vec::new();
    if let Some(selector) = json_optional_string(args, "selector") {
        rest.push("--selector".to_string());
        rest.push(selector);
    }
    if let Some(id) = json_optional_string(args, "id") {
        rest.push("--id".to_string());
        rest.push(id);
    }
    if let Some(ref_type) = json_optional_string(args, "type") {
        rest.push("--type".to_string());
        rest.push(ref_type);
    }
    if let Some(section) = json_i64(args, "section")? {
        rest.push("--section".to_string());
        rest.push(section.to_string());
    }
    Ok(rest)
}
