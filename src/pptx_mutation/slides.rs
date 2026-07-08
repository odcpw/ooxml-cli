use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::cli_args::{parse_string_flags, value_flag_present};
use crate::{
    CliError, CliResult, RelationshipEntry, add_relationship_to_xml, allocate_relationship_id,
    attr, attr_exact, copy_zip_with_binary_part_overrides_and_removals,
    copy_zip_with_part_overrides_and_removals, ensure_content_type_override, local_name,
    package_mutation_temp_path, package_type, relationship_entries_from_xml,
    relationship_target_from_source_to_target, relationships_part_for, replace_xml_span,
    resolve_relationship_target, validate, validate_xlsx_mutation_output_flags, xml_attr_escape,
    xml_direct_child_ranges, xml_escape, zip_entry_names, zip_text,
};

mod output;

use self::output::{
    clone_slide_destination, clone_slide_result_json, delete_result_json, move_result_json,
    moved_slide_destination, new_slide_from_layout_result_json, reorder_result_json,
};

const NOTES_REL_TYPE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesSlide";
const SLIDE_REL_TYPE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide";
const SLIDE_LAYOUT_REL_TYPE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout";
const IMAGE_REL_TYPE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";
const SLIDE_CONTENT_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.slide+xml";
const NOTES_CONTENT_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.notesSlide+xml";

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
    binary_overrides: BTreeMap<String, Vec<u8>>,
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

#[derive(Clone, Copy)]
struct ElementSpan {
    start: usize,
    end: usize,
}

#[derive(Clone, Copy)]
struct ShapeBounds {
    x: i64,
    y: i64,
    cx: i64,
    cy: i64,
}

struct CloneSlideMutation {
    package: PptxPackageMutation,
    source_slide: i64,
    insert_after: i64,
    slide_count_before: usize,
    slide_count_after: usize,
    new_slide_number: i64,
    new_slide_id: u32,
    new_slide_uri: String,
    notes_uri: String,
}

struct CloneNotesForSlideContext<'a> {
    file: &'a str,
    allocated_entries: &'a mut Vec<String>,
    content_types: &'a mut String,
    overrides: &'a mut BTreeMap<String, String>,
}

struct NewSlideFromLayoutMutation {
    package: PptxPackageMutation,
    layout: String,
    requested_insert_after: i64,
    new_slide_number: i64,
    new_slide_id: u32,
    new_slide_uri: String,
}

#[derive(Clone)]
struct TextShapeTarget {
    span: ElementSpan,
    tx_body: Option<ElementSpan>,
    shape_id: u32,
    shape_name: String,
    placeholder_type: String,
    placeholder_index: Option<u32>,
    bounds: Option<ShapeBounds>,
}

struct ImageSlotAssignment {
    target: String,
    image_path: String,
}

struct ImageSlotPayload {
    image_part: String,
    content_type: String,
    data: Vec<u8>,
}

#[derive(Clone, Copy)]
struct ImageSlotPackage<'a> {
    file: &'a str,
    slide_part: &'a str,
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

pub(crate) fn pptx_clone_slide(file: &str, args: &[String]) -> CliResult<Value> {
    let slide = crate::parse_i64_flag(args, "--slide")?.unwrap_or(0);
    if slide < 1 {
        return Err(CliError::invalid_args("--slide must be >= 1"));
    }
    let insert_after = crate::parse_i64_flag(args, "--insert-after")?.unwrap_or(0);
    let options = parse_slide_mutation_options(args)?;
    ensure_pptx(file)?;
    let mutation = build_clone_slide_mutation(file, slide, insert_after)?;
    let output_path = slide_mutation_output_path(file, &options);
    let staged_path = stage_slide_mutation(file, &mutation.package, &options)?;
    let source = clone_slide_destination(file, mutation.source_slide, Some(file))?;
    let destination = clone_slide_destination(
        &staged_path,
        mutation.new_slide_number,
        output_path.as_deref(),
    )?;
    let result = clone_slide_result_json(
        file,
        &mutation,
        output_path.as_deref(),
        options.dry_run,
        source,
        destination,
    );
    finish_slide_mutation(file, &staged_path, &options, output_path.as_deref())?;
    Ok(result)
}

pub(crate) fn pptx_new_slide_from_layout(file: &str, args: &[String]) -> CliResult<Value> {
    let layout = crate::parse_string_flag(args, "--layout")?
        .ok_or_else(|| CliError::invalid_args("--layout must be specified"))?;
    if layout.trim().is_empty() {
        return Err(CliError::invalid_args("--layout must be specified"));
    }
    reject_deferred_new_slide_flags(args)?;
    let insert_after = crate::parse_i64_flag(args, "--insert-after")?.unwrap_or(0);
    let set_texts = parse_text_assignments(&parse_string_flags(args, "--set-text")?)?;
    let image_slots = parse_image_slot_assignments(&parse_string_flags(args, "--set-image-slot")?)?;
    let image_fit = normalize_image_fit(
        crate::parse_string_flag(args, "--image-fit")?
            .as_deref()
            .unwrap_or("contain"),
    )?;
    let options = parse_slide_mutation_options(args)?;
    ensure_pptx(file)?;
    let mutation = build_new_slide_from_layout_mutation(
        file,
        &layout,
        insert_after,
        &set_texts,
        &image_slots,
        &image_fit,
    )?;
    let output_path = slide_mutation_output_path(file, &options);
    let staged_path = stage_slide_mutation(file, &mutation.package, &options)?;
    let destination = clone_slide_destination(
        &staged_path,
        mutation.new_slide_number,
        output_path.as_deref(),
    )?;
    let result = new_slide_from_layout_result_json(
        file,
        &mutation,
        output_path.as_deref(),
        options.dry_run,
        destination,
    );
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
            binary_overrides: BTreeMap::new(),
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
            binary_overrides: BTreeMap::new(),
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
            binary_overrides: BTreeMap::new(),
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
            binary_overrides: BTreeMap::new(),
            removals: BTreeSet::new(),
        },
        new_order: parsed,
        slide_count,
    })
}

fn build_clone_slide_mutation(
    file: &str,
    slide: i64,
    insert_after: i64,
) -> CliResult<CloneSlideMutation> {
    let presentation_xml = zip_text(file, "ppt/presentation.xml")?;
    let refs = pptx_slide_refs_for_lifecycle(file, &presentation_xml)?;
    let slide_count = refs.len();
    if slide < 1 || slide as usize > slide_count {
        return Err(CliError::target_not_found(format!(
            "slide {slide} not found"
        )));
    }
    let insert_after = if insert_after == 0 {
        slide
    } else {
        insert_after
    };
    if insert_after < 1 || insert_after as usize > slide_count {
        return Err(CliError::invalid_args(format!(
            "insert-after {insert_after} out of range"
        )));
    }

    let entries = zip_entry_names(file)?;
    let mut allocated_entries = entries.clone();
    let new_slide_part = allocate_numbered_part_name(&entries, "ppt/slides/slide", ".xml");
    allocated_entries.push(new_slide_part.clone());
    let new_slide_uri = package_uri(&new_slide_part);
    let source_ref = &refs[slide as usize - 1];
    let slide_xml = zip_text(file, &source_ref.part)?;
    let source_rels_part = relationships_part_for(&source_ref.part);
    let source_rels_xml = zip_text(file, &source_rels_part).unwrap_or_else(|_| relationships_xml());
    let source_rels = relationship_entries_from_xml(&source_rels_xml);
    let mut content_types = ensure_content_type_override(
        zip_text(file, "[Content_Types].xml")?,
        &new_slide_part,
        SLIDE_CONTENT_TYPE,
    )?;
    let mut overrides = BTreeMap::new();
    let mut cloned_rels = Vec::new();
    let mut cloned_notes_uri = String::new();
    for rel in source_rels {
        if rel.rel_type == NOTES_REL_TYPE {
            let mut notes_ctx = CloneNotesForSlideContext {
                file,
                allocated_entries: &mut allocated_entries,
                content_types: &mut content_types,
                overrides: &mut overrides,
            };
            let (notes_uri, notes_rel) = clone_notes_for_cloned_slide(
                &mut notes_ctx,
                &source_ref.part,
                &new_slide_part,
                &rel,
                &cloned_rels,
            )?;
            cloned_notes_uri = notes_uri;
            cloned_rels.push(notes_rel);
            continue;
        }
        cloned_rels.push(rel);
    }
    let new_slide_rels = render_relationships_xml(&cloned_rels);

    let new_slide_id = next_presentation_slide_id(&presentation_xml);
    let presentation_rels_xml = zip_text(file, "ppt/_rels/presentation.xml.rels")?;
    let presentation_rels = relationship_entries_from_xml(&presentation_rels_xml);
    let new_rel_id = allocate_relationship_id(&presentation_rels);
    let rel_target =
        relationship_target_from_source_to_target("ppt/presentation.xml", &new_slide_part);
    let updated_presentation_rels = add_relationship_to_xml(
        presentation_rels_xml,
        &new_rel_id,
        SLIDE_REL_TYPE,
        &rel_target,
    );
    let new_fragment = format!(r#"<p:sldId id="{new_slide_id}" r:id="{new_rel_id}"/>"#);
    let updated_presentation = insert_slide_fragment(
        &presentation_xml,
        &refs,
        insert_after as usize,
        &new_fragment,
    )?;
    overrides.insert(new_slide_part.clone(), slide_xml);
    overrides.insert(relationships_part_for(&new_slide_part), new_slide_rels);
    overrides.insert("ppt/presentation.xml".to_string(), updated_presentation);
    overrides.insert(
        "ppt/_rels/presentation.xml.rels".to_string(),
        updated_presentation_rels,
    );
    overrides.insert("[Content_Types].xml".to_string(), content_types);

    Ok(CloneSlideMutation {
        package: PptxPackageMutation {
            overrides,
            binary_overrides: BTreeMap::new(),
            removals: BTreeSet::new(),
        },
        source_slide: slide,
        insert_after,
        slide_count_before: slide_count,
        slide_count_after: slide_count + 1,
        new_slide_number: insert_after + 1,
        new_slide_id,
        new_slide_uri,
        notes_uri: cloned_notes_uri,
    })
}

fn build_new_slide_from_layout_mutation(
    file: &str,
    layout_selector: &str,
    insert_after: i64,
    set_texts: &[(String, String)],
    image_slots: &[ImageSlotAssignment],
    image_fit: &str,
) -> CliResult<NewSlideFromLayoutMutation> {
    let layouts = crate::pptx_readback::pptx_presentation_layouts(file)?;
    let layout = crate::pptx_readback::pptx_find_layout(&layouts, layout_selector)
        .ok_or_else(|| CliError::invalid_args(format!("layout {layout_selector:?} not found")))?
        .clone();
    let presentation_xml = zip_text(file, "ppt/presentation.xml")?;
    let refs = pptx_slide_refs_for_lifecycle(file, &presentation_xml)?;
    let slide_count = refs.len();
    let requested_insert_after = insert_after;
    let insert_after = if insert_after == 0 {
        slide_count as i64
    } else {
        insert_after
    };
    if insert_after < 0 || insert_after as usize > slide_count {
        return Err(CliError::invalid_args(format!(
            "insert-after {insert_after} out of range"
        )));
    }

    if let Some(template_slide) = find_template_slide_for_layout(file, &refs, &layout.part_uri)? {
        let mut cloned = build_clone_slide_mutation(file, template_slide, insert_after)?;
        let new_slide_part = cloned.new_slide_uri.trim_start_matches('/').to_string();
        let template_part = &refs[template_slide as usize - 1].part;
        let mut slide_xml = reset_slide_text_bodies(&zip_text(file, template_part)?)?;
        for (target, text) in set_texts {
            slide_xml = set_text_target(&slide_xml, target, text)?;
        }
        let mut slide_rels_xml = cloned
            .package
            .overrides
            .get(&relationships_part_for(&new_slide_part))
            .cloned()
            .unwrap_or_else(|| {
                zip_text(file, &relationships_part_for(&new_slide_part))
                    .unwrap_or_else(|_| relationships_xml())
            });
        let mut content_types_xml = cloned
            .package
            .overrides
            .get("[Content_Types].xml")
            .cloned()
            .unwrap_or_else(|| zip_text(file, "[Content_Types].xml").unwrap_or_default());
        apply_image_slot_assignments(
            ImageSlotPackage {
                file,
                slide_part: &new_slide_part,
            },
            &mut slide_xml,
            &mut slide_rels_xml,
            &mut content_types_xml,
            &mut cloned.package.binary_overrides,
            image_slots,
            image_fit,
        )?;
        cloned
            .package
            .overrides
            .insert(new_slide_part.clone(), slide_xml);
        cloned
            .package
            .overrides
            .insert(relationships_part_for(&new_slide_part), slide_rels_xml);
        cloned
            .package
            .overrides
            .insert("[Content_Types].xml".to_string(), content_types_xml);
        return Ok(NewSlideFromLayoutMutation {
            package: cloned.package,
            layout: layout_selector.to_string(),
            requested_insert_after,
            new_slide_number: cloned.new_slide_number,
            new_slide_id: cloned.new_slide_id,
            new_slide_uri: cloned.new_slide_uri,
        });
    }

    let entries = zip_entry_names(file)?;
    let new_slide_part = allocate_numbered_part_name(&entries, "ppt/slides/slide", ".xml");
    let new_slide_uri = package_uri(&new_slide_part);
    let layout_xml = zip_text(file, layout.part_uri.trim_start_matches('/'))?;
    let c_sld = find_first_element_span(&layout_xml, "cSld")?
        .ok_or_else(|| CliError::unexpected("layout common slide data not found"))?;
    let mut slide_xml =
        build_slide_xml_from_layout_common_data(&layout_xml[c_sld.start..c_sld.end]);
    slide_xml = reset_slide_text_bodies(&slide_xml)?;
    for (target, text) in set_texts {
        slide_xml = set_text_target(&slide_xml, target, text)?;
    }

    let new_slide_id = next_presentation_slide_id(&presentation_xml);
    let presentation_rels_xml = zip_text(file, "ppt/_rels/presentation.xml.rels")?;
    let presentation_rels = relationship_entries_from_xml(&presentation_rels_xml);
    let new_rel_id = allocate_relationship_id(&presentation_rels);
    let slide_rel_target =
        relationship_target_from_source_to_target("ppt/presentation.xml", &new_slide_part);
    let updated_presentation_rels = add_relationship_to_xml(
        presentation_rels_xml,
        &new_rel_id,
        SLIDE_REL_TYPE,
        &slide_rel_target,
    );
    let new_fragment = format!(r#"<p:sldId id="{new_slide_id}" r:id="{new_rel_id}"/>"#);
    let updated_presentation = insert_slide_fragment(
        &presentation_xml,
        &refs,
        insert_after as usize,
        &new_fragment,
    )?;

    let layout_target = relationship_target_from_source_to_target(&new_slide_uri, &layout.part_uri);
    let mut slide_rels_xml = render_relationships_xml(&[RelationshipEntry {
        id: "rId1".to_string(),
        rel_type: SLIDE_LAYOUT_REL_TYPE.to_string(),
        target: layout_target,
        target_mode: String::new(),
    }]);
    let mut content_types = ensure_content_type_override(
        zip_text(file, "[Content_Types].xml")?,
        &new_slide_part,
        SLIDE_CONTENT_TYPE,
    )?;
    let mut binary_overrides = BTreeMap::new();
    apply_image_slot_assignments(
        ImageSlotPackage {
            file,
            slide_part: &new_slide_part,
        },
        &mut slide_xml,
        &mut slide_rels_xml,
        &mut content_types,
        &mut binary_overrides,
        image_slots,
        image_fit,
    )?;

    let mut overrides = BTreeMap::new();
    overrides.insert(new_slide_part.clone(), slide_xml);
    overrides.insert(relationships_part_for(&new_slide_part), slide_rels_xml);
    overrides.insert("ppt/presentation.xml".to_string(), updated_presentation);
    overrides.insert(
        "ppt/_rels/presentation.xml.rels".to_string(),
        updated_presentation_rels,
    );
    overrides.insert("[Content_Types].xml".to_string(), content_types);

    Ok(NewSlideFromLayoutMutation {
        package: PptxPackageMutation {
            overrides,
            binary_overrides,
            removals: BTreeSet::new(),
        },
        layout: layout_selector.to_string(),
        requested_insert_after,
        new_slide_number: insert_after + 1,
        new_slide_id,
        new_slide_uri,
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

fn clone_notes_for_cloned_slide(
    ctx: &mut CloneNotesForSlideContext<'_>,
    source_slide_part: &str,
    new_slide_part: &str,
    source_notes_rel: &RelationshipEntry,
    destination_rels: &[RelationshipEntry],
) -> CliResult<(String, RelationshipEntry)> {
    let source_slide_uri = package_uri(source_slide_part);
    let source_notes_uri = resolve_relationship_target(&source_slide_uri, &source_notes_rel.target);
    let source_notes_part = package_part_name(&source_notes_uri);
    let notes_xml = zip_text(ctx.file, &source_notes_part)?;
    let new_notes_part =
        allocate_numbered_part_name(ctx.allocated_entries, "ppt/notesSlides/notesSlide", ".xml");
    ctx.allocated_entries.push(new_notes_part.clone());
    let new_notes_uri = package_uri(&new_notes_part);

    ctx.overrides.insert(new_notes_part.clone(), notes_xml);
    *ctx.content_types = ensure_content_type_override(
        ctx.content_types.clone(),
        &new_notes_part,
        NOTES_CONTENT_TYPE,
    )?;

    let source_notes_rels_part = relationships_part_for(&source_notes_part);
    if let Ok(source_notes_rels_xml) = zip_text(ctx.file, &source_notes_rels_part) {
        let new_slide_uri = package_uri(new_slide_part);
        let mut new_notes_rels = Vec::new();
        for rel in relationship_entries_from_xml(&source_notes_rels_xml) {
            if rel.target_mode == "External" {
                new_notes_rels.push(rel);
                continue;
            }
            let target_uri = if rel.rel_type == SLIDE_REL_TYPE {
                new_slide_uri.clone()
            } else {
                resolve_relationship_target(&source_notes_uri, &rel.target)
            };
            new_notes_rels.push(RelationshipEntry {
                id: rel.id,
                rel_type: rel.rel_type,
                target: relationship_target_from_source_to_target(&new_notes_uri, &target_uri),
                target_mode: String::new(),
            });
        }
        ctx.overrides.insert(
            relationships_part_for(&new_notes_part),
            render_relationships_xml(&new_notes_rels),
        );
    }

    Ok((
        new_notes_uri.clone(),
        RelationshipEntry {
            id: allocate_relationship_id(destination_rels),
            rel_type: NOTES_REL_TYPE.to_string(),
            target: relationship_target_from_source_to_target(
                &package_uri(new_slide_part),
                &new_notes_uri,
            ),
            target_mode: String::new(),
        },
    ))
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
    if mutation.binary_overrides.is_empty() {
        copy_zip_with_part_overrides_and_removals(
            file,
            &write_path,
            &mutation.overrides,
            &mutation.removals,
        )?;
    } else {
        copy_zip_with_binary_part_overrides_and_removals(
            file,
            &write_path,
            &mutation.overrides,
            &mutation.binary_overrides,
            &mutation.removals,
        )?;
    }
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

fn package_part_name(uri: &str) -> String {
    uri.trim_start_matches('/').to_string()
}

fn relationships_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"></Relationships>"#.to_string()
}

fn render_relationships_xml(rels: &[RelationshipEntry]) -> String {
    let mut out = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
    );
    for rel in rels {
        let target_mode = if rel.target_mode.is_empty() {
            String::new()
        } else {
            format!(r#" TargetMode="{}""#, xml_attr_escape(&rel.target_mode))
        };
        out.push_str(&format!(
            r#"<Relationship Id="{}" Type="{}" Target="{}"{} />"#,
            xml_attr_escape(&rel.id),
            xml_attr_escape(&rel.rel_type),
            xml_attr_escape(&rel.target),
            target_mode
        ));
    }
    out.push_str("</Relationships>");
    out
}

fn allocate_numbered_part_name(entries: &[String], prefix: &str, suffix: &str) -> String {
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
    format!("{prefix}{next}{suffix}")
}

fn package_uri(part: &str) -> String {
    format!("/{}", part.trim_start_matches('/'))
}

fn next_presentation_slide_id(xml: &str) -> u32 {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut max_id = 255_u32;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "sldId" =>
            {
                if let Some(id) = attr(&e, "id").and_then(|value| value.parse::<u32>().ok()) {
                    max_id = max_id.max(id);
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    max_id + 1
}

fn insert_slide_fragment(
    presentation_xml: &str,
    refs: &[SlideIdRef],
    insert_after: usize,
    new_fragment: &str,
) -> CliResult<String> {
    let mut fragments = refs
        .iter()
        .map(|slide| slide.fragment.clone())
        .collect::<Vec<_>>();
    if insert_after > fragments.len() {
        return Err(CliError::invalid_args(format!(
            "insert-after {insert_after} out of range"
        )));
    }
    fragments.insert(insert_after, new_fragment.to_string());
    replace_slide_id_list(presentation_xml, refs, &fragments)
}

fn find_template_slide_for_layout(
    file: &str,
    refs: &[SlideIdRef],
    layout_uri: &str,
) -> CliResult<Option<i64>> {
    let wanted = package_uri(layout_uri);
    for (index, slide_ref) in refs.iter().enumerate() {
        if slide_layout_uri(file, &slide_ref.part)?.as_deref() == Some(wanted.as_str()) {
            return Ok(Some(index as i64 + 1));
        }
    }
    Ok(None)
}

fn slide_layout_uri(file: &str, slide_part: &str) -> CliResult<Option<String>> {
    let rels_xml =
        zip_text(file, &relationships_part_for(slide_part)).unwrap_or_else(|_| relationships_xml());
    for rel in relationship_entries_from_xml(&rels_xml) {
        if rel.rel_type == SLIDE_LAYOUT_REL_TYPE {
            return Ok(Some(resolve_relationship_target(
                &package_uri(slide_part),
                &rel.target,
            )));
        }
    }
    Ok(None)
}

fn build_slide_xml_from_layout_common_data(c_sld_xml: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">{c_sld_xml}<p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>"#
    )
}

fn parse_text_assignments(values: &[String]) -> CliResult<Vec<(String, String)>> {
    let mut assignments = Vec::new();
    for value in values {
        if value.trim().is_empty() || value == "[]" {
            continue;
        }
        let Some((key, text)) = value.split_once('=') else {
            return Err(CliError::invalid_args(format!(
                "invalid --set-text value {value:?} (expected key=value)"
            )));
        };
        let key = key.trim();
        if key.is_empty() {
            return Err(CliError::invalid_args(format!(
                "invalid --set-text value {value:?} (expected key=value)"
            )));
        }
        assignments.push((key.to_string(), text.to_string()));
    }
    Ok(assignments)
}

fn reject_deferred_new_slide_flags(args: &[String]) -> CliResult<()> {
    for name in ["--set-rich-text", "--set-image", "--set-image-coords"] {
        if value_flag_present(args, name) {
            return Err(CliError::invalid_args(format!(
                "pptx new-slide-from-layout {name} is deferred in the Rust port; use --set-text for this slice"
            )));
        }
    }
    for name in [
        "--level",
        "--align",
        "--bullet-mode",
        "--bullet-char",
        "--auto-num",
        "--space-before",
        "--space-after",
        "--line-spacing",
    ] {
        if value_flag_present(args, name) {
            return Err(CliError::invalid_args(format!(
                "pptx new-slide-from-layout {name} is deferred in the Rust port"
            )));
        }
    }
    Ok(())
}

fn parse_image_slot_assignments(values: &[String]) -> CliResult<Vec<ImageSlotAssignment>> {
    let mut assignments = Vec::new();
    for value in values {
        if value.trim().is_empty() || value == "[]" {
            continue;
        }
        let Some((target, image_path)) = value.split_once('=') else {
            return Err(CliError::invalid_args(format!(
                "invalid --set-image-slot value {value:?} (expected key=path)"
            )));
        };
        let target = target.trim();
        let image_path = image_path.trim();
        if target.is_empty() || image_path.is_empty() {
            return Err(CliError::invalid_args(format!(
                "invalid --set-image-slot value {value:?} (expected key=path)"
            )));
        }
        if !Path::new(image_path).exists() {
            return Err(CliError::file_not_found(format!(
                "file not found: {image_path}"
            )));
        }
        assignments.push(ImageSlotAssignment {
            target: target.to_string(),
            image_path: image_path.to_string(),
        });
    }
    Ok(assignments)
}

fn normalize_image_fit(mode: &str) -> CliResult<String> {
    match mode.to_ascii_lowercase().as_str() {
        "contain" | "fit" => Ok("contain".to_string()),
        "cover" | "crop" => Ok("cover".to_string()),
        "stretch" => Ok("stretch".to_string()),
        other => Err(CliError::invalid_args(format!(
            "invalid image fit {other:?} (must be 'cover', 'contain', or 'stretch')"
        ))),
    }
}

fn apply_image_slot_assignments(
    package: ImageSlotPackage<'_>,
    slide_xml: &mut String,
    slide_rels_xml: &mut String,
    content_types_xml: &mut String,
    binary_overrides: &mut BTreeMap<String, Vec<u8>>,
    assignments: &[ImageSlotAssignment],
    image_fit: &str,
) -> CliResult<()> {
    for assignment in assignments {
        let targets = text_shape_targets(slide_xml)?
            .into_iter()
            .filter(|shape| image_slot_matches(shape, &assignment.target))
            .collect::<Vec<_>>();
        let target = match targets.as_slice() {
            [target] => target.clone(),
            [] => {
                return Err(CliError::target_not_found(format!(
                    "target not found: {}",
                    assignment.target
                )));
            }
            _ => {
                return Err(CliError::target_not_found(format!(
                    "ambiguous target: {}",
                    assignment.target
                )));
            }
        };
        let payload =
            load_image_slot_payload(package.file, target.shape_id, &assignment.image_path)?;
        let rels = relationship_entries_from_xml(slide_rels_xml);
        let relationship_id = allocate_relationship_id(&rels);
        let rel_target = relationship_target_from_source_to_target(
            &format!("/{}", package.slide_part),
            &payload.image_part,
        );
        *slide_rels_xml = add_relationship_to_xml(
            slide_rels_xml.clone(),
            &relationship_id,
            IMAGE_REL_TYPE,
            &rel_target,
        );
        *content_types_xml = ensure_content_type_override(
            std::mem::take(content_types_xml),
            &payload.image_part,
            &payload.content_type,
        )?;
        let picture_xml = image_slot_picture_xml(&target, &relationship_id, image_fit)?;
        *slide_xml = replace_xml_span(slide_xml, target.span.start, target.span.end, &picture_xml);
        binary_overrides.insert(
            payload.image_part.trim_start_matches('/').to_string(),
            payload.data,
        );
    }
    Ok(())
}

fn image_slot_matches(shape: &TextShapeTarget, target: &str) -> bool {
    if placeholder_role(&shape.placeholder_type) != "pic" {
        return false;
    }
    text_shape_matches(shape, target)
}

fn load_image_slot_payload(
    file: &str,
    shape_id: u32,
    image_path: &str,
) -> CliResult<ImageSlotPayload> {
    let data = fs::read(image_path)
        .map_err(|err| CliError::unexpected(format!("failed to read image file: {err}")))?;
    let content_type = image_content_type_from_path(image_path)?;
    validate_image_payload(&content_type, &data)?;
    let extension = image_extension_for_content_type(&content_type)?;
    let image_part = allocate_image_part(file, shape_id, extension)?;
    Ok(ImageSlotPayload {
        image_part,
        content_type,
        data,
    })
}

fn image_slot_picture_xml(
    target: &TextShapeTarget,
    rel_id: &str,
    image_fit: &str,
) -> CliResult<String> {
    let bounds = target
        .bounds
        .ok_or_else(|| CliError::unexpected("picture placeholder bounds not found"))?;
    let shape_name = if target.shape_name.is_empty() {
        format!("Picture {}", target.shape_id)
    } else {
        target.shape_name.clone()
    };
    let fit_xml = if image_fit == "cover" {
        r#"<a:tile tx="0" ty="0" sx="100000" sy="100000" flip="none" algn="ctr"/>"#
    } else {
        "<a:stretch><a:fillRect/></a:stretch>"
    };
    Ok(format!(
        r#"<p:pic><p:nvPicPr><p:cNvPr id="{}" name="{}"/><p:cNvPicPr><a:picLocks noChangeAspect="1"/></p:cNvPicPr><p:nvPr/></p:nvPicPr><p:blipFill><a:blip r:embed="{}"/>{fit_xml}</p:blipFill><p:spPr><a:xfrm><a:off x="{}" y="{}"/><a:ext cx="{}" cy="{}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr></p:pic>"#,
        target.shape_id,
        xml_attr_escape(&shape_name),
        xml_attr_escape(rel_id),
        bounds.x,
        bounds.y,
        bounds.cx,
        bounds.cy
    ))
}

fn image_content_type_from_path(path: &str) -> CliResult<String> {
    let ext = Path::new(path)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match ext.as_str() {
        "png" => Ok("image/png".to_string()),
        "jpg" | "jpeg" => Ok("image/jpeg".to_string()),
        "gif" => Ok("image/gif".to_string()),
        "bmp" => Ok("image/bmp".to_string()),
        "tif" | "tiff" => Ok("image/tiff".to_string()),
        _ => Err(CliError::invalid_args(format!(
            "unsupported image content type for {path:?}"
        ))),
    }
}

fn image_extension_for_content_type(content_type: &str) -> CliResult<&'static str> {
    match content_type {
        "image/png" => Ok(".png"),
        "image/jpeg" => Ok(".jpeg"),
        "image/gif" => Ok(".gif"),
        "image/bmp" => Ok(".bmp"),
        "image/tiff" => Ok(".tiff"),
        _ => Err(CliError::invalid_args(format!(
            "unsupported image content type {content_type:?}"
        ))),
    }
}

fn validate_image_payload(content_type: &str, data: &[u8]) -> CliResult<()> {
    let ok = match content_type {
        "image/png" => data.starts_with(b"\x89PNG\r\n\x1a\n"),
        "image/jpeg" => data.starts_with(&[0xff, 0xd8, 0xff]),
        "image/gif" => data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a"),
        _ => true,
    };
    if ok {
        Ok(())
    } else {
        Err(CliError::invalid_args(format!(
            "image payload does not match content type {content_type}"
        )))
    }
}

fn allocate_image_part(file: &str, shape_id: u32, extension: &str) -> CliResult<String> {
    let entries = zip_entry_names(file)?;
    let base = format!("/ppt/media/image{shape_id}");
    let mut candidate = format!("{base}{extension}");
    let mut counter = 1_u32;
    while entries
        .iter()
        .any(|entry| format!("/{}", entry.trim_start_matches('/')) == candidate)
    {
        candidate = format!("{base}_{counter}{extension}");
        counter += 1;
    }
    Ok(candidate)
}

fn reset_slide_text_bodies(xml: &str) -> CliResult<String> {
    let mut out = xml.to_string();
    let shapes = text_shape_targets(xml)?;
    for shape in shapes.into_iter().rev() {
        if let Some(tx_body) = shape.tx_body {
            out = replace_xml_span(&out, tx_body.start, tx_body.end, &text_body_xml(""));
        }
    }
    Ok(out)
}

fn set_text_target(xml: &str, target: &str, text: &str) -> CliResult<String> {
    let shapes = text_shape_targets(xml)?;
    let matches = shapes
        .iter()
        .filter(|shape| text_shape_matches(shape, target))
        .cloned()
        .collect::<Vec<_>>();
    let shape = match matches.as_slice() {
        [shape] => shape,
        [] => {
            return Err(CliError::target_not_found(format!(
                "target not found: {target}"
            )));
        }
        _ => {
            return Err(CliError::target_not_found(format!(
                "ambiguous target: {target}"
            )));
        }
    };
    let replacement = text_body_xml(text);
    if let Some(tx_body) = shape.tx_body {
        return Ok(replace_xml_span(
            xml,
            tx_body.start,
            tx_body.end,
            &replacement,
        ));
    }
    let insert_at = shape
        .span
        .end
        .checked_sub(close_tag_len(xml, shape.span.end)?)
        .ok_or_else(|| CliError::unexpected("invalid shape span"))?;
    Ok(insert_xml_at(xml, insert_at, &replacement))
}

fn text_shape_targets(xml: &str) -> CliResult<Vec<TextShapeTarget>> {
    let Some(sp_tree) = find_first_element_span(xml, "spTree")? else {
        return Ok(Vec::new());
    };
    let (content_start, content_end) = element_content_bounds(&xml[sp_tree.start..sp_tree.end])?;
    let ranges = xml_direct_child_ranges(
        xml,
        sp_tree.start + content_start,
        sp_tree.start + content_end,
    )?;
    let mut out = Vec::new();
    for range in ranges.into_iter().filter(|range| range.kind == "sp") {
        let fragment = &xml[range.start..range.end];
        let tx_body = find_first_element_span(fragment, "txBody")?.map(|span| ElementSpan {
            start: range.start + span.start,
            end: range.start + span.end,
        });
        let mut target = TextShapeTarget {
            span: ElementSpan {
                start: range.start,
                end: range.end,
            },
            tx_body,
            shape_id: 0,
            shape_name: String::new(),
            placeholder_type: String::new(),
            placeholder_index: None,
            bounds: None,
        };
        let mut reader = Reader::from_str(fragment);
        reader.config_mut().trim_text(true);
        loop {
            match reader.read_event() {
                Ok(Event::Start(e)) | Ok(Event::Empty(e))
                    if local_name(e.name().as_ref()) == "cNvPr" =>
                {
                    target.shape_id = attr(&e, "id")
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(target.shape_id);
                    target.shape_name =
                        attr(&e, "name").unwrap_or_else(|| target.shape_name.clone());
                }
                Ok(Event::Start(e)) | Ok(Event::Empty(e))
                    if local_name(e.name().as_ref()) == "ph" =>
                {
                    target.placeholder_type = attr(&e, "type").unwrap_or_default();
                    target.placeholder_index = attr(&e, "idx").and_then(|value| value.parse().ok());
                }
                Ok(Event::Start(e)) | Ok(Event::Empty(e))
                    if local_name(e.name().as_ref()) == "off" =>
                {
                    let mut bounds = target.bounds.unwrap_or(ShapeBounds {
                        x: 0,
                        y: 0,
                        cx: 0,
                        cy: 0,
                    });
                    bounds.x = attr(&e, "x")
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(bounds.x);
                    bounds.y = attr(&e, "y")
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(bounds.y);
                    target.bounds = Some(bounds);
                }
                Ok(Event::Start(e)) | Ok(Event::Empty(e))
                    if local_name(e.name().as_ref()) == "ext" =>
                {
                    let mut bounds = target.bounds.unwrap_or(ShapeBounds {
                        x: 0,
                        y: 0,
                        cx: 0,
                        cy: 0,
                    });
                    bounds.cx = attr(&e, "cx")
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(bounds.cx);
                    bounds.cy = attr(&e, "cy")
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(bounds.cy);
                    target.bounds = Some(bounds);
                }
                Ok(Event::Eof) => break,
                Err(err) => return Err(CliError::unexpected(err.to_string())),
                _ => {}
            }
        }
        out.push(target);
    }
    Ok(out)
}

fn text_shape_matches(shape: &TextShapeTarget, target: &str) -> bool {
    let target = target.trim();
    if target == format!("shape:{}", shape.shape_id) {
        return true;
    }
    if !shape.shape_name.is_empty() && target == format!("~{}", shape.shape_name) {
        return true;
    }
    let role = placeholder_role(&shape.placeholder_type);
    if !role.is_empty() && target == role {
        return true;
    }
    if let Some(index) = shape.placeholder_index
        && (target == format!("{role}:{index}") || target == format!("#{index}"))
    {
        return true;
    }
    false
}

fn placeholder_role(literal_type: &str) -> String {
    match literal_type {
        "ctrTitle" | "title" => "title",
        "subTitle" => "subtitle",
        "body" | "obj" => "body",
        other => other,
    }
    .to_string()
}

fn text_body_xml(text: &str) -> String {
    format!(
        "<p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>{}</a:t></a:r></a:p></p:txBody>",
        xml_escape(text)
    )
}

fn insert_xml_at(xml: &str, index: usize, insert: &str) -> String {
    let mut out = String::with_capacity(xml.len() + insert.len());
    out.push_str(&xml[..index]);
    out.push_str(insert);
    out.push_str(&xml[index..]);
    out
}

fn close_tag_len(xml: &str, end: usize) -> CliResult<usize> {
    let prefix = &xml[..end];
    let close_start = prefix
        .rfind("</")
        .ok_or_else(|| CliError::unexpected("invalid PPTX shape XML"))?;
    Ok(end - close_start)
}

fn find_first_element_span(xml: &str, wanted_local: &str) -> CliResult<Option<ElementSpan>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut active: Option<(usize, usize)> = None;
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                if let Some((_, depth)) = active.as_mut() {
                    *depth += 1;
                } else if local_name(e.name().as_ref()) == wanted_local {
                    active = Some((before, 1));
                }
            }
            Ok(Event::Empty(e)) => {
                if active.is_none() && local_name(e.name().as_ref()) == wanted_local {
                    return Ok(Some(ElementSpan {
                        start: before,
                        end: reader.buffer_position() as usize,
                    }));
                }
            }
            Ok(Event::End(e)) => {
                if let Some((start, depth)) = active.as_mut() {
                    if *depth == 1 && local_name(e.name().as_ref()) == wanted_local {
                        return Ok(Some(ElementSpan {
                            start: *start,
                            end: reader.buffer_position() as usize,
                        }));
                    }
                    *depth = depth.saturating_sub(1);
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(None)
}

fn element_content_bounds(fragment: &str) -> CliResult<(usize, usize)> {
    let open_end = fragment
        .find('>')
        .ok_or_else(|| CliError::unexpected("invalid PPTX XML"))?;
    if fragment[..=open_end].trim_end().ends_with("/>") {
        return Ok((open_end + 1, open_end + 1));
    }
    let close_start = fragment
        .rfind("</")
        .ok_or_else(|| CliError::unexpected("invalid PPTX XML"))?;
    Ok((open_end + 1, close_start))
}
