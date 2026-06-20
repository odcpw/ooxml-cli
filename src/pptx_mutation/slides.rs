use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use crate::{
    CliError, CliResult, attr, attr_exact, command_arg, copy_zip_with_part_overrides_and_removals,
    local_name, package_mutation_temp_path, package_type, pptx_slides_list,
    relationship_entries_from_xml, relationships_part_for, replace_xml_span,
    resolve_relationship_target, validate, validate_xlsx_mutation_output_flags, zip_text,
};

const NOTES_REL_TYPE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesSlide";

#[derive(Clone)]
struct PptxSlideMutationOptions {
    out: Option<String>,
    backup: Option<String>,
    dry_run: bool,
    in_place: bool,
    no_validate: bool,
}

struct PptxPackageMutation {
    overrides: BTreeMap<String, String>,
    removals: BTreeSet<String>,
}

#[derive(Clone)]
struct SlideIdRef {
    rel_id: String,
    part: String,
    fragment: String,
}

struct SlideIdSpan {
    rel_id: String,
    start: usize,
    end: usize,
}

struct ElementSpan {
    start: usize,
    end: usize,
}

pub(crate) fn pptx_slides_delete(file: &str, slide: i64, args: &[String]) -> CliResult<Value> {
    let options = parse_slide_mutation_options(args)?;
    ensure_pptx(file)?;
    let mutation = build_delete_slide_mutation(file, slide)?;
    let output_path = slide_mutation_output_path(file, &options);
    let staged_path = stage_slide_mutation(file, &mutation.package, &options)?;
    let result = delete_result_json(file, &mutation, output_path.as_deref(), options.dry_run);
    finish_slide_mutation(file, &staged_path, &options, output_path.as_deref())?;
    Ok(result)
}

pub(crate) fn pptx_slides_move(
    file: &str,
    from_position: i64,
    to_position: i64,
    args: &[String],
) -> CliResult<Value> {
    let options = parse_slide_mutation_options(args)?;
    ensure_pptx(file)?;
    let mutation = build_move_slide_mutation(file, from_position, to_position)?;
    let output_path = slide_mutation_output_path(file, &options);
    let staged_path = stage_slide_mutation(file, &mutation.package, &options)?;
    let destination =
        moved_slide_destination(&staged_path, mutation.to_position, output_path.as_deref())?;
    let result = move_result_json(
        file,
        &mutation,
        output_path.as_deref(),
        options.dry_run,
        destination,
    );
    finish_slide_mutation(file, &staged_path, &options, output_path.as_deref())?;
    Ok(result)
}

pub(crate) fn pptx_slides_reorder(file: &str, order: &str, args: &[String]) -> CliResult<Value> {
    let options = parse_slide_mutation_options(args)?;
    ensure_pptx(file)?;
    let mutation = build_reorder_slides_mutation(file, order)?;
    let output_path = slide_mutation_output_path(file, &options);
    let staged_path = stage_slide_mutation(file, &mutation.package, &options)?;
    let result = reorder_result_json(file, &mutation, output_path.as_deref(), options.dry_run);
    finish_slide_mutation(file, &staged_path, &options, output_path.as_deref())?;
    Ok(result)
}

struct DeleteSlideMutation {
    package: PptxPackageMutation,
    deleted_slide: i64,
    removed_uri: String,
    removed_notes: Option<String>,
    remaining_slides: usize,
}

struct MoveSlideMutation {
    package: PptxPackageMutation,
    slide_uri: String,
    from_position: i64,
    to_position: i64,
    is_no_op: bool,
}

struct ReorderSlidesMutation {
    package: PptxPackageMutation,
    new_order: Vec<i64>,
    slide_count: usize,
}

fn parse_slide_mutation_options(args: &[String]) -> CliResult<PptxSlideMutationOptions> {
    let out = crate::parse_string_flag(args, "--out")?;
    let backup = crate::parse_string_flag(args, "--backup")?;
    let dry_run = crate::has_flag(args, "--dry-run");
    let in_place = crate::has_flag(args, "--in-place");
    let no_validate = crate::has_flag(args, "--no-validate");
    validate_xlsx_mutation_output_flags(out.as_deref(), in_place, backup.as_deref(), dry_run)?;
    Ok(PptxSlideMutationOptions {
        out,
        backup,
        dry_run,
        in_place,
        no_validate,
    })
}

fn ensure_pptx(file: &str) -> CliResult<()> {
    let detected = package_type(file)?;
    if detected != "pptx" {
        return Err(CliError::unsupported_type(format!(
            "file is not a PPTX document (detected: {detected})"
        )));
    }
    Ok(())
}

fn build_delete_slide_mutation(file: &str, slide: i64) -> CliResult<DeleteSlideMutation> {
    let presentation_xml = zip_text(file, "ppt/presentation.xml")?;
    let refs = pptx_slide_refs_for_lifecycle(file, &presentation_xml)?;
    let slide_count = refs.len();
    if slide < 1 || slide as usize > slide_count {
        return Err(CliError::invalid_args(format!(
            "slide number {slide} out of range (presentation has {slide_count} slides)"
        )));
    }

    let index = slide as usize - 1;
    let slide_ref = &refs[index];
    let mut next_fragments: Vec<String> = refs.iter().map(|slide| slide.fragment.clone()).collect();
    next_fragments.remove(index);

    let updated_presentation = replace_slide_id_list(&presentation_xml, &refs, &next_fragments)?;
    let pres_rels_xml = zip_text(file, "ppt/_rels/presentation.xml.rels")?;
    let updated_pres_rels = remove_relationship_by_id(&pres_rels_xml, &slide_ref.rel_id);

    let slide_rels_part = relationships_part_for(&slide_ref.part);
    let slide_rels_xml = zip_text(file, &slide_rels_part).unwrap_or_else(|_| relationships_xml());
    let removed_notes = related_part_by_type(&slide_ref.part, &slide_rels_xml, NOTES_REL_TYPE);

    let mut removals = BTreeSet::new();
    removals.insert(slide_ref.part.clone());
    removals.insert(slide_rels_part);
    if let Some(notes_part) = removed_notes.as_deref() {
        removals.insert(notes_part.to_string());
        removals.insert(relationships_part_for(notes_part));
    }

    let mut removed_content_types = BTreeSet::new();
    removed_content_types.insert(slide_ref.part.clone());
    if let Some(notes_part) = removed_notes.as_deref() {
        removed_content_types.insert(notes_part.to_string());
    }
    let content_types = zip_text(file, "[Content_Types].xml")?;
    let updated_content_types =
        remove_content_type_overrides(&content_types, &removed_content_types);

    let mut overrides = BTreeMap::new();
    overrides.insert("ppt/presentation.xml".to_string(), updated_presentation);
    overrides.insert(
        "ppt/_rels/presentation.xml.rels".to_string(),
        updated_pres_rels,
    );
    overrides.insert("[Content_Types].xml".to_string(), updated_content_types);

    Ok(DeleteSlideMutation {
        package: PptxPackageMutation {
            overrides,
            removals,
        },
        deleted_slide: slide,
        removed_uri: format!("/{}", slide_ref.part),
        removed_notes: removed_notes.map(|part| format!("/{part}")),
        remaining_slides: slide_count.saturating_sub(1),
    })
}

fn build_move_slide_mutation(
    file: &str,
    from_position: i64,
    to_position: i64,
) -> CliResult<MoveSlideMutation> {
    let presentation_xml = zip_text(file, "ppt/presentation.xml")?;
    let refs = pptx_slide_refs_for_lifecycle(file, &presentation_xml)?;
    let slide_count = refs.len();
    if from_position < 1 || from_position as usize > slide_count {
        return Err(CliError::invalid_args(format!(
            "from-position {from_position} out of range (presentation has {slide_count} slides)"
        )));
    }
    if to_position < 1 || to_position as usize > slide_count {
        return Err(CliError::invalid_args(format!(
            "to-position {to_position} out of range (valid range: 1-{slide_count})"
        )));
    }

    let from_index = from_position as usize - 1;
    let to_index = to_position as usize - 1;
    let slide_uri = format!("/{}", refs[from_index].part);
    let is_no_op = from_index == to_index;
    let mut fragments: Vec<String> = refs.iter().map(|slide| slide.fragment.clone()).collect();
    if !is_no_op {
        let moved = fragments.remove(from_index);
        fragments.insert(to_index, moved);
    }

    let package = if is_no_op {
        PptxPackageMutation {
            overrides: BTreeMap::new(),
            removals: BTreeSet::new(),
        }
    } else {
        let mut overrides = BTreeMap::new();
        overrides.insert(
            "ppt/presentation.xml".to_string(),
            replace_slide_id_list(&presentation_xml, &refs, &fragments)?,
        );
        PptxPackageMutation {
            overrides,
            removals: BTreeSet::new(),
        }
    };

    Ok(MoveSlideMutation {
        package,
        slide_uri,
        from_position,
        to_position,
        is_no_op,
    })
}

fn build_reorder_slides_mutation(file: &str, order: &str) -> CliResult<ReorderSlidesMutation> {
    let presentation_xml = zip_text(file, "ppt/presentation.xml")?;
    let refs = pptx_slide_refs_for_lifecycle(file, &presentation_xml)?;
    let slide_count = refs.len();
    let parsed = parse_slide_permutation(order, slide_count)?;
    let fragments = parsed
        .iter()
        .map(|position| refs[*position as usize - 1].fragment.clone())
        .collect::<Vec<_>>();
    let mut overrides = BTreeMap::new();
    overrides.insert(
        "ppt/presentation.xml".to_string(),
        replace_slide_id_list(&presentation_xml, &refs, &fragments)?,
    );
    Ok(ReorderSlidesMutation {
        package: PptxPackageMutation {
            overrides,
            removals: BTreeSet::new(),
        },
        new_order: parsed,
        slide_count,
    })
}

fn parse_slide_permutation(order: &str, slide_count: usize) -> CliResult<Vec<i64>> {
    let parts: Vec<&str> = order.split(',').collect();
    if parts.len() != slide_count {
        return Err(CliError::invalid_args(format!(
            "permutation has {} elements but presentation has {slide_count} slides",
            parts.len()
        )));
    }

    let mut seen = BTreeSet::new();
    let mut parsed = Vec::with_capacity(parts.len());
    for part in parts {
        let trimmed = part.trim();
        let value = trimmed.parse::<i64>().map_err(|err| {
            CliError::unexpected(format!(
                "failed to reorder slides: failed to reorder slides: invalid slide position {trimmed}: {err}"
            ))
        })?;
        if value < 1 || value as usize > slide_count {
            return Err(CliError::unexpected(format!(
                "failed to reorder slides: failed to reorder slides: slide position {value} out of range (valid range: 1-{slide_count})"
            )));
        }
        if !seen.insert(value) {
            return Err(CliError::unexpected(format!(
                "failed to reorder slides: failed to reorder slides: duplicate slide position {value} in permutation"
            )));
        }
        parsed.push(value);
    }
    Ok(parsed)
}

fn pptx_slide_refs_for_lifecycle(file: &str, presentation_xml: &str) -> CliResult<Vec<SlideIdRef>> {
    let spans = slide_id_spans(presentation_xml)?;
    let rels = relationship_entries_from_xml(&zip_text(file, "ppt/_rels/presentation.xml.rels")?);
    spans
        .into_iter()
        .map(|span| {
            let rel = rels
                .iter()
                .find(|candidate| candidate.id == span.rel_id)
                .ok_or_else(|| {
                    CliError::unexpected(format!("missing relationship {}", span.rel_id))
                })?;
            Ok(SlideIdRef {
                rel_id: span.rel_id,
                part: package_part_name(&resolve_relationship_target(
                    "/ppt/presentation.xml",
                    &rel.target,
                )),
                fragment: presentation_xml[span.start..span.end].to_string(),
            })
        })
        .collect()
}

fn slide_id_spans(xml: &str) -> CliResult<Vec<SlideIdSpan>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut spans = Vec::new();
    let mut current: Option<(usize, String, usize)> = None;
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == "sldId" => {
                if let Some(span) =
                    slide_id_span_from_attrs(xml, before, reader.buffer_position() as usize, &e)
                {
                    spans.push(span);
                }
            }
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "sldId" => {
                if let (Some(id), Some(rel_id)) = (attr_exact(&e, "id"), attr_exact(&e, "r:id"))
                    && id.parse::<u32>().is_ok()
                {
                    current = Some((before, rel_id, 1));
                }
            }
            Ok(Event::Start(_)) => {
                if let Some((_, _, depth)) = current.as_mut() {
                    *depth += 1;
                }
            }
            Ok(Event::End(e)) => {
                if let Some((start, rel_id, depth)) = current.as_mut() {
                    if *depth == 1 && local_name(e.name().as_ref()) == "sldId" {
                        spans.push(SlideIdSpan {
                            rel_id: rel_id.clone(),
                            start: *start,
                            end: reader.buffer_position() as usize,
                        });
                        current = None;
                    } else {
                        *depth = depth.saturating_sub(1);
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(spans)
}

fn slide_id_span_from_attrs(
    xml: &str,
    start: usize,
    end: usize,
    e: &quick_xml::events::BytesStart<'_>,
) -> Option<SlideIdSpan> {
    attr_exact(e, "id")?.parse::<u32>().ok()?;
    let rel_id = attr_exact(e, "r:id")?;
    Some(SlideIdSpan {
        rel_id,
        start,
        end: end.min(xml.len()),
    })
}

fn replace_slide_id_list(
    presentation_xml: &str,
    refs: &[SlideIdRef],
    fragments: &[String],
) -> CliResult<String> {
    let first = refs
        .first()
        .ok_or_else(|| CliError::unexpected("presentation has no slides"))?;
    let last = refs
        .last()
        .ok_or_else(|| CliError::unexpected("presentation has no slides"))?;
    let first_start = presentation_xml
        .find(&first.fragment)
        .ok_or_else(|| CliError::unexpected("slide list span not found"))?;
    let last_start = presentation_xml
        .find(&last.fragment)
        .ok_or_else(|| CliError::unexpected("slide list span not found"))?;
    let replacement = fragments.join("");
    Ok(replace_xml_span(
        presentation_xml,
        first_start,
        last_start + last.fragment.len(),
        &replacement,
    ))
}

fn remove_relationship_by_id(xml: &str, rel_id: &str) -> String {
    remove_elements_matching(xml, "Relationship", "Id", rel_id)
}

fn remove_content_type_overrides(xml: &str, parts: &BTreeSet<String>) -> String {
    let mut out = xml.to_string();
    for part in parts {
        let part_name = format!("/{}", part.trim_start_matches('/'));
        out = remove_elements_matching(&out, "Override", "PartName", &part_name);
    }
    out
}

fn remove_elements_matching(xml: &str, local: &str, attr_name: &str, attr_value: &str) -> String {
    let spans = matching_element_spans(xml, local, attr_name, attr_value);
    let mut out = xml.to_string();
    for span in spans.into_iter().rev() {
        out = replace_xml_span(&out, span.start, span.end, "");
    }
    out
}

fn matching_element_spans(
    xml: &str,
    wanted_local: &str,
    attr_name: &str,
    attr_value: &str,
) -> Vec<ElementSpan> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut spans = Vec::new();
    let mut current: Option<(usize, usize)> = None;
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == wanted_local
                    && attr(&e, attr_name).as_deref() == Some(attr_value) =>
            {
                spans.push(ElementSpan {
                    start: before,
                    end: reader.buffer_position() as usize,
                });
            }
            Ok(Event::Start(e))
                if current.is_none()
                    && local_name(e.name().as_ref()) == wanted_local
                    && attr(&e, attr_name).as_deref() == Some(attr_value) =>
            {
                current = Some((before, 1));
            }
            Ok(Event::Start(_)) => {
                if let Some((_, depth)) = current.as_mut() {
                    *depth += 1;
                }
            }
            Ok(Event::End(e)) => {
                if let Some((start, depth)) = current.as_mut() {
                    if *depth == 1 && local_name(e.name().as_ref()) == wanted_local {
                        spans.push(ElementSpan {
                            start: *start,
                            end: reader.buffer_position() as usize,
                        });
                        current = None;
                    } else {
                        *depth = depth.saturating_sub(1);
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    spans
}

fn related_part_by_type(source_part: &str, rels_xml: &str, rel_type: &str) -> Option<String> {
    relationship_entries_from_xml(rels_xml)
        .into_iter()
        .find(|rel| rel.rel_type == rel_type)
        .map(|rel| {
            package_part_name(&resolve_relationship_target(
                &format!("/{}", source_part.trim_start_matches('/')),
                &rel.target,
            ))
        })
}

fn stage_slide_mutation(
    file: &str,
    mutation: &PptxPackageMutation,
    options: &PptxSlideMutationOptions,
) -> CliResult<String> {
    let output_path = options
        .out
        .as_deref()
        .filter(|value| !value.trim().is_empty());
    let write_path = if options.dry_run || options.in_place || output_path == Some(file) {
        package_mutation_temp_path(file, "pptx-slides")
    } else {
        output_path
            .ok_or_else(|| {
                CliError::invalid_args(
                    "must specify exactly one of --out, --in-place, or --dry-run",
                )
            })?
            .to_string()
    };
    copy_zip_with_part_overrides_and_removals(
        file,
        &write_path,
        &mutation.overrides,
        &mutation.removals,
    )?;
    if !options.no_validate {
        validate(&write_path, true)?;
    }
    Ok(write_path)
}

fn finish_slide_mutation(
    file: &str,
    staged_path: &str,
    options: &PptxSlideMutationOptions,
    output_path: Option<&str>,
) -> CliResult<()> {
    if options.dry_run {
        let _ = fs::remove_file(staged_path);
    } else if options.in_place || output_path == Some(file) {
        if let Some(backup_path) = options
            .backup
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            fs::copy(file, backup_path)
                .map_err(|err| CliError::unexpected(format!("failed to create backup: {err}")))?;
        }
        fs::rename(staged_path, file)
            .or_else(|_| {
                fs::copy(staged_path, file)?;
                fs::remove_file(staged_path)
            })
            .map_err(|err| CliError::unexpected(format!("failed to write output file: {err}")))?;
    }
    Ok(())
}

fn slide_mutation_output_path(file: &str, options: &PptxSlideMutationOptions) -> Option<String> {
    if options.dry_run {
        None
    } else if options.in_place {
        Some(file.to_string())
    } else {
        options
            .out
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(ToString::to_string)
    }
}

fn moved_slide_destination(
    readback_file: &str,
    slide: i64,
    output_path: Option<&str>,
) -> CliResult<Value> {
    let list = pptx_slides_list(readback_file)?;
    let item = list
        .get("slides")
        .and_then(Value::as_array)
        .and_then(|slides| slides.get(slide as usize - 1))
        .ok_or_else(|| CliError::unexpected(format!("slide {slide} readback not found")))?;
    let mut out = Map::new();
    if let Some(output_path) = output_path {
        out.insert("file".to_string(), json!(output_path));
    }
    copy_json_field(item, &mut out, "number");
    copy_json_field(item, &mut out, "partUri");
    copy_non_empty_string_field(item, &mut out, "layout");
    copy_non_empty_string_field(item, &mut out, "layoutPartUri");
    copy_non_empty_string_field(item, &mut out, "notesPartUri");
    copy_json_field(item, &mut out, "textShapes");
    copy_json_field(item, &mut out, "images");
    copy_json_field(item, &mut out, "tables");
    copy_json_field(item, &mut out, "notes");
    Ok(Value::Object(out))
}

fn copy_json_field(source: &Value, target: &mut Map<String, Value>, key: &str) {
    if let Some(value) = source.get(key) {
        target.insert(key.to_string(), value.clone());
    }
}

fn copy_non_empty_string_field(source: &Value, target: &mut Map<String, Value>, key: &str) {
    if let Some(value) = source.get(key).and_then(Value::as_str)
        && !value.is_empty()
    {
        target.insert(key.to_string(), json!(value));
    }
}

fn delete_result_json(
    file: &str,
    mutation: &DeleteSlideMutation,
    output_path: Option<&str>,
    dry_run: bool,
) -> Value {
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    if let Some(output_path) = output_path {
        result.insert("output".to_string(), json!(output_path));
    }
    result.insert("dryRun".to_string(), json!(dry_run));
    result.insert("deletedSlide".to_string(), json!(mutation.deleted_slide));
    result.insert("removedUri".to_string(), json!(mutation.removed_uri));
    if let Some(removed_notes) = mutation.removed_notes.as_deref() {
        result.insert("removedNotes".to_string(), json!(removed_notes));
    }
    result.insert(
        "remainingSlides".to_string(),
        json!(mutation.remaining_slides),
    );
    add_pptx_slides_mutation_commands(&mut result, output_path);
    Value::Object(result)
}

fn move_result_json(
    file: &str,
    mutation: &MoveSlideMutation,
    output_path: Option<&str>,
    dry_run: bool,
    destination: Value,
) -> Value {
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    if let Some(output_path) = output_path {
        result.insert("output".to_string(), json!(output_path));
    }
    result.insert("dryRun".to_string(), json!(dry_run));
    result.insert("slideUri".to_string(), json!(mutation.slide_uri));
    result.insert("fromPosition".to_string(), json!(mutation.from_position));
    result.insert("toPosition".to_string(), json!(mutation.to_position));
    result.insert("isNoOp".to_string(), json!(mutation.is_no_op));
    result.insert("destination".to_string(), destination);
    add_slide_readback_command(&mut result, output_path, mutation.to_position);
    add_pptx_slides_mutation_commands(&mut result, output_path);
    Value::Object(result)
}

fn reorder_result_json(
    file: &str,
    mutation: &ReorderSlidesMutation,
    output_path: Option<&str>,
    dry_run: bool,
) -> Value {
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    if let Some(output_path) = output_path {
        result.insert("output".to_string(), json!(output_path));
    }
    result.insert("dryRun".to_string(), json!(dry_run));
    result.insert("newOrder".to_string(), json!(mutation.new_order));
    result.insert("slideCount".to_string(), json!(mutation.slide_count));
    add_pptx_slides_mutation_commands(&mut result, output_path);
    Value::Object(result)
}

fn add_slide_readback_command(
    result: &mut Map<String, Value>,
    output_path: Option<&str>,
    slide: i64,
) {
    let command_target = output_path.unwrap_or("<out.pptx>");
    let command_suffix = if output_path.is_some() {
        ""
    } else {
        "Template"
    };
    result.insert(
        format!("readbackCommand{command_suffix}"),
        json!(format!(
            "ooxml --json pptx slides show {} --slide {slide} --include-text --include-bounds",
            command_arg(command_target)
        )),
    );
}

fn add_pptx_slides_mutation_commands(result: &mut Map<String, Value>, output_path: Option<&str>) {
    let command_target = output_path.unwrap_or("<out.pptx>");
    let command_suffix = if output_path.is_some() {
        ""
    } else {
        "Template"
    };
    result.insert(
        format!("slidesListCommand{command_suffix}"),
        json!(format!(
            "ooxml --json pptx slides list {}",
            command_arg(command_target)
        )),
    );
    result.insert(
        format!("validateCommand{command_suffix}"),
        json!(format!(
            "ooxml validate --strict {}",
            command_arg(command_target)
        )),
    );
    result.insert(
        format!("renderCommand{command_suffix}"),
        json!(format!(
            "ooxml pptx render {} --out render-check",
            command_arg(command_target)
        )),
    );
}

fn package_part_name(uri: &str) -> String {
    uri.trim_start_matches('/').to_string()
}

fn relationships_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"></Relationships>"#.to_string()
}
