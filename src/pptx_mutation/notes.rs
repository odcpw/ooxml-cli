use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::fs;

use crate::{
    CliError, CliResult, RelationshipEntry, add_relationship_to_xml, allocate_relationship_id,
    attr, attr_exact, command_arg, copy_zip_with_part_overrides, ensure_content_type_override,
    local_name, package_mutation_temp_path, package_type, relationship_entries_from_xml,
    relationship_target_from_source_to_target, relationships_part_for, resolve_relationship_target,
    validate, validate_xlsx_mutation_output_flags, xml_direct_child_ranges, xml_escape, zip_text,
};

const NOTES_REL_TYPE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesSlide";
const NOTES_MASTER_REL_TYPE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesMaster";
const SLIDE_REL_TYPE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide";
const THEME_REL_TYPE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme";
const NOTES_CONTENT_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.notesSlide+xml";
const NOTES_MASTER_CONTENT_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.notesMaster+xml";

pub(crate) fn pptx_notes_set(file: &str, args: &[String]) -> CliResult<Value> {
    let slide = parse_notes_slide(args)?;
    let text = crate::parse_string_flag(args, "--text")?
        .ok_or_else(|| CliError::invalid_args("required flag(s) \"text\" not set"))?;
    let options = parse_notes_mutation_options(args)?;
    pptx_notes_set_text(file, slide, &text, options)
}

pub(crate) fn pptx_notes_clear(file: &str, args: &[String]) -> CliResult<Value> {
    let slide = parse_notes_slide(args)?;
    let options = parse_notes_mutation_options(args)?;
    pptx_notes_set_text(file, slide, "", options)
}

fn parse_notes_slide(args: &[String]) -> CliResult<u32> {
    let slide = crate::parse_i64_flag(args, "--slide")?
        .ok_or_else(|| CliError::invalid_args("required flag(s) \"slide\" not set"))?;
    if slide < 1 {
        return Err(CliError::invalid_args("--slide must be >= 1"));
    }
    Ok(slide as u32)
}

#[derive(Clone)]
struct PptxNotesMutationOptions {
    out: Option<String>,
    backup: Option<String>,
    dry_run: bool,
    in_place: bool,
    no_validate: bool,
}

fn parse_notes_mutation_options(args: &[String]) -> CliResult<PptxNotesMutationOptions> {
    let out = crate::parse_string_flag(args, "--out")?;
    let backup = crate::parse_string_flag(args, "--backup")?;
    let dry_run = crate::has_flag(args, "--dry-run");
    let in_place = crate::has_flag(args, "--in-place");
    let no_validate = crate::has_flag(args, "--no-validate");
    validate_xlsx_mutation_output_flags(out.as_deref(), in_place, backup.as_deref(), dry_run)?;
    Ok(PptxNotesMutationOptions {
        out,
        backup,
        dry_run,
        in_place,
        no_validate,
    })
}

fn pptx_notes_set_text(
    file: &str,
    slide: u32,
    text: &str,
    options: PptxNotesMutationOptions,
) -> CliResult<Value> {
    let detected = package_type(file)?;
    if detected != "pptx" {
        return Err(CliError::unsupported_type(format!(
            "file is not a PPTX document (detected: {detected})"
        )));
    }
    let mutation = build_notes_mutation(file, slide, text)?;
    write_notes_mutation(file, &mutation.overrides, &options)?;
    Ok(notes_mutation_result_json(file, &mutation.result, &options))
}

struct PptxNotesMutation {
    result: SetNotesResult,
    overrides: BTreeMap<String, String>,
}

struct SetNotesResult {
    slide: u32,
    slide_part_uri: String,
    notes_uri: String,
    text: String,
    created_part: bool,
    created_relationship: bool,
}

fn build_notes_mutation(file: &str, slide: u32, text: &str) -> CliResult<PptxNotesMutation> {
    let slides = pptx_slide_refs_for_mutation(file)?;
    let slide_ref = slides.get(slide as usize - 1).ok_or_else(|| {
        CliError::invalid_args(format!(
            "slide {slide} not found (presentation has {} slides)",
            slides.len()
        ))
    })?;

    let mut overrides = BTreeMap::new();
    let mut content_types = zip_text(file, "[Content_Types].xml")?;
    let slide_rels_part = relationships_part_for(&slide_ref.part);
    let slide_rels_xml = zip_text(file, &slide_rels_part).unwrap_or_else(|_| relationships_xml());
    let slide_rels = relationship_entries_from_xml(&slide_rels_xml);
    let existing_notes_uri =
        relationship_target_by_type(&slide_ref.part, &slide_rels, NOTES_REL_TYPE);

    let (notes_uri, created_part, created_relationship) = if let Some(notes_uri) =
        existing_notes_uri
    {
        (notes_uri, false, false)
    } else {
        let entries = crate::zip_entry_names(file)?;
        let notes_uri = allocate_numbered_part_name(
            &entries,
            "ppt/notesSlides/notesSlide",
            ".xml",
            "ppt/notesSlides/notesSlide",
        );
        content_types =
            ensure_content_type_override(content_types, &notes_uri, NOTES_CONTENT_TYPE)?;

        let mut notes_master_uri = find_presentation_related_part(file, NOTES_MASTER_REL_TYPE)?;
        if notes_master_uri.is_none() {
            let master_uri = allocate_numbered_part_name(
                &entries,
                "ppt/notesMasters/notesMaster",
                ".xml",
                "ppt/notesMasters/notesMaster",
            );
            content_types = ensure_content_type_override(
                content_types,
                &master_uri,
                NOTES_MASTER_CONTENT_TYPE,
            )?;
            overrides.insert(master_uri.clone(), create_notes_master_document());
            if let Some(theme_uri) = find_presentation_related_part(file, THEME_REL_TYPE)? {
                let target = relationship_target_from_source_to_target(&master_uri, &theme_uri);
                let rels_xml =
                    append_allocated_relationship(relationships_xml(), THEME_REL_TYPE, &target);
                overrides.insert(relationships_part_for(&master_uri), rels_xml);
            }

            let pres_rels_part = relationships_part_for("ppt/presentation.xml");
            let pres_rels_xml =
                zip_text(file, &pres_rels_part).unwrap_or_else(|_| relationships_xml());
            let target =
                relationship_target_from_source_to_target("ppt/presentation.xml", &master_uri);
            overrides.insert(
                pres_rels_part,
                append_allocated_relationship(pres_rels_xml, NOTES_MASTER_REL_TYPE, &target),
            );
            notes_master_uri = Some(master_uri);
        }

        let mut notes_rels_xml = relationships_xml();
        if let Some(master_uri) = notes_master_uri.as_deref() {
            let target = relationship_target_from_source_to_target(&notes_uri, master_uri);
            notes_rels_xml =
                append_allocated_relationship(notes_rels_xml, NOTES_MASTER_REL_TYPE, &target);
        }
        let slide_target = relationship_target_from_source_to_target(&notes_uri, &slide_ref.part);
        notes_rels_xml =
            append_allocated_relationship(notes_rels_xml, SLIDE_REL_TYPE, &slide_target);
        overrides.insert(relationships_part_for(&notes_uri), notes_rels_xml);

        let notes_target = relationship_target_from_source_to_target(&slide_ref.part, &notes_uri);
        overrides.insert(
            slide_rels_part.clone(),
            append_allocated_relationship(slide_rels_xml, NOTES_REL_TYPE, &notes_target),
        );
        overrides.insert(notes_uri.clone(), create_notes_slide_document());
        (notes_uri, true, true)
    };

    let notes_xml = overrides
        .get(&notes_uri)
        .cloned()
        .unwrap_or_else(|| zip_text(file, &notes_uri).unwrap_or_default());
    let updated_notes_xml = set_notes_body_text(&notes_xml, text)?;
    overrides.insert(notes_uri.clone(), updated_notes_xml);
    overrides.insert("[Content_Types].xml".to_string(), content_types);

    Ok(PptxNotesMutation {
        result: SetNotesResult {
            slide,
            slide_part_uri: format!("/{}", slide_ref.part),
            notes_uri: format!("/{notes_uri}"),
            text: text.to_string(),
            created_part,
            created_relationship,
        },
        overrides,
    })
}

#[derive(Clone)]
struct PptxSlideRef {
    part: String,
}

fn pptx_slide_refs_for_mutation(file: &str) -> CliResult<Vec<PptxSlideRef>> {
    let presentation = zip_text(file, "ppt/presentation.xml")?;
    let slide_refs = presentation_slide_refs(&presentation);
    let rels = relationship_entries_from_xml(&zip_text(file, "ppt/_rels/presentation.xml.rels")?);
    slide_refs
        .into_iter()
        .map(|rel_id| {
            let rel = rels
                .iter()
                .find(|candidate| candidate.id == rel_id)
                .ok_or_else(|| CliError::unexpected(format!("missing relationship {rel_id}")))?;
            Ok(PptxSlideRef {
                part: package_part_name(&resolve_relationship_target(
                    "/ppt/presentation.xml",
                    &rel.target,
                )),
            })
        })
        .collect()
}

fn presentation_slide_refs(xml: &str) -> Vec<String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut slides = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "sldId" =>
            {
                if let Some(rel) = attr_exact(&e, "r:id") {
                    slides.push(rel);
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    slides
}

fn relationship_target_by_type(
    source_part: &str,
    rels: &[RelationshipEntry],
    rel_type: &str,
) -> Option<String> {
    rels.iter()
        .find(|rel| rel.rel_type == rel_type)
        .map(|rel| package_part_name(&resolve_relationship_target(source_part, &rel.target)))
}

fn find_presentation_related_part(file: &str, rel_type: &str) -> CliResult<Option<String>> {
    let rels_part = relationships_part_for("ppt/presentation.xml");
    let rels_xml = zip_text(file, &rels_part).unwrap_or_else(|_| relationships_xml());
    let rels = relationship_entries_from_xml(&rels_xml);
    Ok(relationship_target_by_type(
        "/ppt/presentation.xml",
        &rels,
        rel_type,
    ))
}

fn allocate_numbered_part_name(
    entries: &[String],
    prefix: &str,
    suffix: &str,
    output_prefix: &str,
) -> String {
    let mut next = 1_u32;
    for entry in entries {
        let normalized = entry.trim_start_matches('/');
        if let Some(raw) = normalized
            .strip_prefix(prefix)
            .and_then(|tail| tail.strip_suffix(suffix))
            && let Ok(value) = raw.parse::<u32>()
            && value >= next
        {
            next = value + 1;
        }
    }
    format!("{output_prefix}{next}{suffix}")
}

fn append_allocated_relationship(xml: String, rel_type: &str, target: &str) -> String {
    let rels = relationship_entries_from_xml(&xml);
    let id = allocate_relationship_id(&rels);
    add_relationship_to_xml(xml, &id, rel_type, target)
}

fn set_notes_body_text(xml: &str, text: &str) -> CliResult<String> {
    let shape = find_body_placeholder_shape_span(xml).ok_or_else(|| {
        CliError::invalid_args("notes slide has no body placeholder shape to hold notes text")
    })?;
    if let Some((content_start, content_end)) = find_element_content_span(xml, shape, "txBody")? {
        let children = xml_direct_child_ranges(xml, content_start, content_end)?;
        let mut replacement = String::new();
        let mut cursor = content_start;
        for child in children {
            if child.kind == "p" {
                replacement.push_str(&xml[cursor..child.start]);
                cursor = child.end;
            }
        }
        replacement.push_str(&xml[cursor..content_end]);
        replacement.push_str(&render_notes_paragraphs(text));
        Ok(crate::replace_xml_span(
            xml,
            content_start,
            content_end,
            &replacement,
        ))
    } else {
        let shape_fragment = &xml[shape.start..shape.end];
        let close_start = shape_fragment
            .rfind("</")
            .ok_or_else(|| CliError::unexpected("invalid notes body placeholder shape"))?;
        Ok(crate::replace_xml_span(
            xml,
            shape.start + close_start,
            shape.start + close_start,
            &render_notes_text_body(text),
        ))
    }
}

#[derive(Clone, Copy)]
struct XmlSpan {
    start: usize,
    end: usize,
}

fn find_body_placeholder_shape_span(xml: &str) -> Option<XmlSpan> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut current: Option<(usize, usize, bool)> = None;
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if let Some((_, depth, has_body)) = current.as_mut() {
                    if name == "ph" && attr(&e, "type").as_deref() == Some("body") {
                        *has_body = true;
                    }
                    *depth += 1;
                } else if name == "sp" {
                    current = Some((before, 1, false));
                }
            }
            Ok(Event::Empty(e)) => {
                if let Some((_, _, has_body)) = current.as_mut()
                    && local_name(e.name().as_ref()) == "ph"
                    && attr(&e, "type").as_deref() == Some("body")
                {
                    *has_body = true;
                }
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if let Some((start, depth, has_body)) = current.as_mut() {
                    if *depth == 1 && name == "sp" {
                        let span = XmlSpan {
                            start: *start,
                            end: reader.buffer_position() as usize,
                        };
                        let matched = *has_body;
                        current = None;
                        if matched {
                            return Some(span);
                        }
                    } else {
                        *depth = depth.saturating_sub(1);
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    None
}

fn find_element_content_span(
    xml: &str,
    outer: XmlSpan,
    wanted_local: &str,
) -> CliResult<Option<(usize, usize)>> {
    let fragment = &xml[outer.start..outer.end];
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(true);
    let mut depth = 0_usize;
    let mut open_end = 0_usize;
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) if depth == 0 && local_name(e.name().as_ref()) == wanted_local => {
                open_end = reader.buffer_position() as usize;
                depth = 1;
            }
            Ok(Event::Start(_)) if depth > 0 => {
                depth += 1;
            }
            Ok(Event::End(e)) if depth > 0 => {
                if depth == 1 && local_name(e.name().as_ref()) == wanted_local {
                    return Ok(Some((outer.start + open_end, outer.start + before)));
                }
                depth -= 1;
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(None)
}

fn render_notes_text_body(text: &str) -> String {
    format!(
        "<p:txBody><a:bodyPr/><a:lstStyle/>{}</p:txBody>",
        render_notes_paragraphs(text)
    )
}

fn render_notes_paragraphs(text: &str) -> String {
    if text.is_empty() {
        return "<a:p/>".to_string();
    }
    text.split('\n')
        .map(|line| {
            if line.is_empty() {
                "<a:p/>".to_string()
            } else {
                format!("<a:p><a:r><a:t>{}</a:t></a:r></a:p>", xml_escape(line))
            }
        })
        .collect::<Vec<_>>()
        .join("")
}

fn create_notes_slide_document() -> String {
    r#"<p:notes xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/><p:sp><p:nvSpPr><p:cNvPr id="2" name="Notes Placeholder 1"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph type="body" idx="1"/></p:nvPr></p:nvSpPr><p:spPr/><p:txBody><a:bodyPr/><a:lstStyle/><a:p/></p:txBody></p:sp></p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:notes>"#.to_string()
}

fn create_notes_master_document() -> String {
    r#"<p:notesMaster xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/></p:spTree></p:cSld><p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/></p:notesMaster>"#.to_string()
}

fn relationships_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"></Relationships>"#.to_string()
}

fn package_part_name(uri: &str) -> String {
    uri.trim_start_matches('/').to_string()
}

fn write_notes_mutation(
    file: &str,
    overrides: &BTreeMap<String, String>,
    options: &PptxNotesMutationOptions,
) -> CliResult<()> {
    let output_path = options
        .out
        .as_deref()
        .filter(|value| !value.trim().is_empty());
    let write_path = if options.dry_run || options.in_place || output_path == Some(file) {
        package_mutation_temp_path(file, "pptx-notes")
    } else {
        output_path
            .ok_or_else(|| {
                CliError::invalid_args(
                    "must specify exactly one of --out, --in-place, or --dry-run",
                )
            })?
            .to_string()
    };

    copy_zip_with_part_overrides(file, &write_path, overrides)?;
    if !options.no_validate {
        validate(&write_path, true)?;
    }
    if options.dry_run {
        let _ = fs::remove_file(&write_path);
    } else if options.in_place || output_path == Some(file) {
        if let Some(backup_path) = options
            .backup
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            fs::copy(file, backup_path)
                .map_err(|err| CliError::unexpected(format!("failed to create backup: {err}")))?;
        }
        fs::rename(&write_path, file)
            .or_else(|_| {
                fs::copy(&write_path, file)?;
                fs::remove_file(&write_path)
            })
            .map_err(|err| CliError::unexpected(format!("failed to write output file: {err}")))?;
    }
    Ok(())
}

fn notes_mutation_result_json(
    file: &str,
    result: &SetNotesResult,
    options: &PptxNotesMutationOptions,
) -> Value {
    let output_path = if options.in_place {
        Some(file)
    } else {
        options
            .out
            .as_deref()
            .filter(|value| !value.trim().is_empty())
    };
    let command_target = if options.dry_run {
        "<out.pptx>"
    } else {
        output_path.unwrap_or(file)
    };
    let command_suffix = if options.dry_run { "Template" } else { "" };
    let mut out = Map::new();
    out.insert("file".to_string(), json!(file));
    if !options.dry_run
        && let Some(output_path) = output_path
    {
        out.insert("output".to_string(), json!(output_path));
    }
    out.insert("dryRun".to_string(), json!(options.dry_run));
    out.insert(
        format!("readbackCommand{command_suffix}"),
        json!(format!(
            "ooxml --json pptx notes show {} --slide {}",
            command_arg(command_target),
            result.slide
        )),
    );
    out.insert(
        format!("slideReadbackCommand{command_suffix}"),
        json!(format!(
            "ooxml --json pptx slides show {} --slide {} --include-text --include-bounds",
            command_arg(command_target),
            result.slide
        )),
    );
    out.insert(
        format!("validateCommand{command_suffix}"),
        json!(format!(
            "ooxml validate --strict {}",
            command_arg(command_target)
        )),
    );
    out.insert(
        format!("renderCommand{command_suffix}"),
        json!(format!(
            "ooxml pptx render {} --out render-check",
            command_arg(command_target)
        )),
    );
    out.insert("slide".to_string(), json!(result.slide));
    out.insert("slidePartUri".to_string(), json!(result.slide_part_uri));
    out.insert("notesUri".to_string(), json!(result.notes_uri));
    out.insert("text".to_string(), json!(result.text));
    out.insert("createdPart".to_string(), json!(result.created_part));
    out.insert(
        "createdRelationship".to_string(),
        json!(result.created_relationship),
    );
    Value::Object(out)
}
