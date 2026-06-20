use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use crate::pptx_readback::animations::{
    AnimationEffectInfo, ParagraphRange, ShapeTarget, SlideRef, XmlSpan,
    animation_effects_for_slide, build_shape_index, classify_effect, click_step_spans,
    collect_shape_targets, direct_child_span, direct_children, element_attr_i64,
    element_content_bounds, find_descendant_element_spans, find_first_element_span, pptx_slide_ref,
    pptx_slide_refs_resolved,
};
use crate::{
    CliError, CliResult, command_arg, copy_zip_with_part_override, copy_zip_with_part_overrides,
    has_flag, package_mutation_temp_path, package_type, parse_i64_flag, parse_string_flag,
    remove_xml_span, replace_xml_span, validate, validate_xlsx_mutation_output_flags,
    xml_attr_escape, xml_escape, zip_text,
};

#[derive(Clone)]
struct PptxAnimationMutationOptions {
    out: Option<String>,
    backup: Option<String>,
    dry_run: bool,
    in_place: bool,
    no_validate: bool,
}

#[derive(Clone)]
struct ResolvedAnimationTarget {
    slide: SlideRef,
    shape: ShapeTarget,
}

struct AddAnimationMutation {
    slide: SlideRef,
    updated_xml: String,
    shape_id: i64,
    shape_name: String,
    effect: String,
    start: String,
    added_effect_ids: Vec<i64>,
    click_step_id: i64,
    created_timing: bool,
    by_paragraph: bool,
    paragraph_count: usize,
}

struct AddAnimationSpec {
    slide: i64,
    shape_selector: String,
    effect: String,
    direction: String,
    duration_ms: i64,
    start: String,
    by_paragraph: bool,
    paragraph_range: Option<ParagraphRange>,
    expect_shape_name: String,
    expect_paragraph_count: i64,
}

struct RemoveAnimationMutation {
    slide: SlideRef,
    updated_xml: String,
    removed_effect_id: i64,
    removed_click_step: bool,
    shape_id: i64,
    shape_name: String,
}

struct ReorderAnimationMutation {
    slide: SlideRef,
    updated_xml: String,
    order: Vec<i64>,
    click_step_count: usize,
}

#[derive(Clone)]
struct PrunedNode {
    slide: u32,
    kind: String,
    effect_id: i64,
    spid: i64,
    stale_reason: String,
}

struct PruneAnimationMutation {
    overrides: BTreeMap<String, String>,
    pruned: Vec<PrunedNode>,
}

struct EffectBuildSpec<'a> {
    effect: &'a str,
    direction: &'a str,
    duration_ms: i64,
    spid: i64,
    paragraph_range: Option<&'a ParagraphRange>,
    effect_id: i64,
    behavior_base_id: i64,
    start: &'a str,
}

pub(crate) fn pptx_animations_add(file: &str, args: &[String]) -> CliResult<Value> {
    ensure_pptx(file)?;
    let options = parse_animation_mutation_options(args)?;
    let slide = parse_i64_flag(args, "--slide")?.unwrap_or(0);
    let shape = parse_string_flag(args, "--shape")?
        .ok_or_else(|| CliError::invalid_args("--shape is required (e.g. shape:4 or ~Title)"))?;
    if shape.trim().is_empty() {
        return Err(CliError::invalid_args(
            "--shape is required (e.g. shape:4 or ~Title)",
        ));
    }
    let effect = normalize_effect(&parse_string_flag(args, "--effect")?.ok_or_else(|| {
        CliError::invalid_args("unknown effect \"\" (expected appear|fade|wipe|flyIn)")
    })?)?;
    let direction = parse_string_flag(args, "--direction")?
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "up".to_string());
    validate_direction(&effect, &direction)?;
    let duration_ms = parse_i64_flag(args, "--duration-ms")?.unwrap_or(500);
    let duration_ms = if duration_ms <= 0 { 500 } else { duration_ms };
    let start = normalize_start(
        &parse_string_flag(args, "--start")?
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "onClick".to_string()),
    )?;
    let by_paragraph = has_flag(args, "--by-paragraph");
    let paragraph_range_text = parse_string_flag(args, "--paragraph-range")?.unwrap_or_default();
    if by_paragraph && !paragraph_range_text.trim().is_empty() {
        return Err(CliError::invalid_args(
            "--by-paragraph and --paragraph-range are mutually exclusive",
        ));
    }
    let paragraph_range = if paragraph_range_text.trim().is_empty() {
        None
    } else {
        Some(parse_paragraph_range(&paragraph_range_text)?)
    };
    let expect_shape_name = parse_string_flag(args, "--expect-shape-name")?.unwrap_or_default();
    let expect_paragraph_count = parse_i64_flag(args, "--expect-paragraph-count")?.unwrap_or(0);

    let spec = AddAnimationSpec {
        slide,
        shape_selector: shape,
        effect,
        direction,
        duration_ms,
        start,
        by_paragraph,
        paragraph_range,
        expect_shape_name,
        expect_paragraph_count,
    };
    let mutation = build_add_animation_mutation(file, &spec)?;
    let output_path = animation_mutation_output_path(file, &options);
    let staged_path =
        stage_animation_part_mutation(file, &mutation.slide.part, &mutation.updated_xml, &options)?;
    let result = add_animation_result(file, &mutation, output_path.as_deref(), options.dry_run);
    finish_animation_mutation(file, &staged_path, &options, output_path.as_deref())?;
    Ok(result)
}

pub(crate) fn pptx_animations_remove(file: &str, args: &[String]) -> CliResult<Value> {
    ensure_pptx(file)?;
    let options = parse_animation_mutation_options(args)?;
    let slide = parse_i64_flag(args, "--slide")?.unwrap_or(0);
    if slide < 1 {
        return Err(CliError::invalid_args("--slide must be >= 1"));
    }
    let effect_id = parse_i64_flag(args, "--effect-id")?.unwrap_or(0);
    if effect_id <= 0 {
        return Err(CliError::invalid_args(
            "--effect-id is required and must be > 0",
        ));
    }
    let expect_shape_name = parse_string_flag(args, "--expect-shape-name")?.unwrap_or_default();
    let mutation =
        build_remove_animation_mutation(file, slide as u32, effect_id, &expect_shape_name)
            .map_err(|err| {
                if err.code == "target_not_found" {
                    animation_effect_not_found_error(file, slide as u32, effect_id)
                } else {
                    err
                }
            })?;
    let output_path = animation_mutation_output_path(file, &options);
    let staged_path =
        stage_animation_part_mutation(file, &mutation.slide.part, &mutation.updated_xml, &options)?;
    let result = remove_animation_result(file, &mutation, output_path.as_deref(), options.dry_run);
    finish_animation_mutation(file, &staged_path, &options, output_path.as_deref())?;
    Ok(result)
}

pub(crate) fn pptx_animations_reorder(file: &str, args: &[String]) -> CliResult<Value> {
    ensure_pptx(file)?;
    let options = parse_animation_mutation_options(args)?;
    let slide = parse_i64_flag(args, "--slide")?.unwrap_or(0);
    if slide < 1 {
        return Err(CliError::invalid_args("--slide must be >= 1"));
    }
    let order_text = parse_string_flag(args, "--order")?.unwrap_or_default();
    let order = parse_int_list(&order_text)?;
    if order.is_empty() {
        return Err(CliError::invalid_args(
            "--order is required (comma-separated clickEffect ids)",
        ));
    }
    let mutation = build_reorder_animation_mutation(file, slide as u32, order)?;
    let output_path = animation_mutation_output_path(file, &options);
    let staged_path =
        stage_animation_part_mutation(file, &mutation.slide.part, &mutation.updated_xml, &options)?;
    let result = reorder_animation_result(file, &mutation, output_path.as_deref(), options.dry_run);
    finish_animation_mutation(file, &staged_path, &options, output_path.as_deref())?;
    Ok(result)
}

pub(crate) fn pptx_animations_prune_stale(file: &str, args: &[String]) -> CliResult<Value> {
    ensure_pptx(file)?;
    let options = parse_animation_mutation_options(args)?;
    let slide = parse_i64_flag(args, "--slide")?.unwrap_or(0);
    if slide < 0 {
        return Err(CliError::invalid_args("--slide must be >= 0"));
    }
    let mutation = build_prune_animation_mutation(file, slide as u32, options.dry_run)?;
    let output_path = animation_mutation_output_path(file, &options);
    if !options.dry_run {
        let staged_path = stage_animation_package_mutation(file, &mutation.overrides, &options)?;
        let result = prune_animation_result(
            file,
            slide as u32,
            &mutation,
            output_path.as_deref(),
            options.dry_run,
        );
        finish_animation_mutation(file, &staged_path, &options, output_path.as_deref())?;
        Ok(result)
    } else {
        Ok(prune_animation_result(
            file,
            slide as u32,
            &mutation,
            None,
            true,
        ))
    }
}

fn build_add_animation_mutation(
    file: &str,
    spec: &AddAnimationSpec,
) -> CliResult<AddAnimationMutation> {
    let target = resolve_animation_target(file, spec.slide, &spec.shape_selector)?;
    if !spec.expect_shape_name.is_empty() && spec.expect_shape_name != target.shape.name {
        return Err(CliError::invalid_args(format!(
            "shape name guard failed: expected {:?} but resolved {:?}",
            spec.expect_shape_name, target.shape.name
        )));
    }
    let slide_xml = zip_text(file, &target.slide.part)?;
    let timing = find_first_element_span(&slide_xml, "timing")?;
    let mut ranges = Vec::new();
    let mut paragraph_count = 0usize;
    if spec.by_paragraph {
        paragraph_count = target.shape.para_count;
        if paragraph_count == 0 {
            return Err(CliError::invalid_args(format!(
                "shape {:?} has no paragraphs to build",
                target.shape.name
            )));
        }
        if spec.expect_paragraph_count > 0
            && spec.expect_paragraph_count as usize != paragraph_count
        {
            return Err(CliError::invalid_args(format!(
                "paragraph count guard failed: expected {} but found {paragraph_count}",
                spec.expect_paragraph_count
            )));
        }
        for index in 0..paragraph_count {
            ranges.push(Some(ParagraphRange {
                start: index as i64,
                end: index as i64,
            }));
        }
    } else if let Some(range) = spec.paragraph_range.as_ref() {
        ranges.push(Some(range.clone()));
    } else {
        ranges.push(None);
    }

    let created_timing = timing.is_none();
    let mut added_effect_ids = Vec::new();
    let mut next_id = timing
        .map(|span| max_timing_node_id(&slide_xml[span.start..span.end]) + 1)
        .unwrap_or(3);
    let mut effect_pars = String::new();
    for (index, range) in ranges.iter().enumerate() {
        let effect_start = if index > 0 {
            "afterPrevious"
        } else {
            &spec.start
        };
        let behavior_count = if matches!(spec.effect.as_str(), "wipe" | "flyIn") {
            2
        } else {
            1
        };
        let effect_id = next_id;
        let behavior_base_id = next_id + 1;
        next_id += 1 + behavior_count;
        effect_pars.push_str(&wrap_effect_par(&build_effect_ctn(&EffectBuildSpec {
            effect: &spec.effect,
            direction: &spec.direction,
            duration_ms: spec.duration_ms,
            spid: target.shape.id,
            paragraph_range: range.as_ref(),
            effect_id,
            behavior_base_id,
            start: effect_start,
        })));
        added_effect_ids.push(effect_id);
    }
    let click_step_id = added_effect_ids.first().copied().unwrap_or_default();
    let mut updated_xml = if let Some(timing) = timing {
        append_effect_pars_to_existing_timing(&slide_xml, timing, &effect_pars)?
    } else {
        insert_created_timing(&slide_xml, &effect_pars)?
    };
    if spec.by_paragraph {
        updated_xml = ensure_build_by_paragraph(&updated_xml, target.shape.id)?;
    }
    Ok(AddAnimationMutation {
        slide: target.slide,
        updated_xml,
        shape_id: target.shape.id,
        shape_name: target.shape.name,
        effect: spec.effect.clone(),
        start: spec.start.clone(),
        added_effect_ids,
        click_step_id,
        created_timing,
        by_paragraph: spec.by_paragraph,
        paragraph_count,
    })
}

fn build_remove_animation_mutation(
    file: &str,
    slide: u32,
    effect_id: i64,
    expect_shape_name: &str,
) -> CliResult<RemoveAnimationMutation> {
    let slide_ref = pptx_slide_ref(file, slide)?;
    let slide_xml = zip_text(file, &slide_ref.part)?;
    let Some(timing) = find_first_element_span(&slide_xml, "timing")? else {
        return Err(CliError::target_not_found(format!(
            "no animations on slide (effect id {effect_id} not found)"
        )));
    };
    let shape_index =
        build_shape_index(&slide_xml, find_first_element_span(&slide_xml, "spTree")?)?;
    let (effect_span, remove_span, info) =
        find_effect_for_mutation(&slide_xml, timing, effect_id, &shape_index)?;
    if !info.supported {
        return Err(CliError::target_not_found(format!(
            "effect id {effect_id} is not a supported entrance effect (refusing to delete preserved/unsupported XML)"
        )));
    }
    if !expect_shape_name.is_empty() && expect_shape_name != info.shape_name {
        return Err(CliError::invalid_args(format!(
            "shape name guard failed: expected {expect_shape_name:?} but effect targets {:?}",
            info.shape_name
        )));
    }
    let _ = effect_span;
    let updated_xml = remove_empty_child_tn_lsts(&remove_xml_span(
        &slide_xml,
        remove_span.start,
        remove_span.end,
    ))?;
    Ok(RemoveAnimationMutation {
        slide: slide_ref,
        updated_xml,
        removed_effect_id: effect_id,
        removed_click_step: true,
        shape_id: info.spid,
        shape_name: info.shape_name,
    })
}

fn build_reorder_animation_mutation(
    file: &str,
    slide: u32,
    order: Vec<i64>,
) -> CliResult<ReorderAnimationMutation> {
    let slide_ref = pptx_slide_ref(file, slide)?;
    let slide_xml = zip_text(file, &slide_ref.part)?;
    let Some(timing) = find_first_element_span(&slide_xml, "timing")? else {
        return Err(CliError::invalid_args("no animations on slide to reorder"));
    };
    let Some(main_seq) = find_main_seq_for_mutation(&slide_xml, timing)? else {
        return Err(CliError::invalid_args("no main sequence found to reorder"));
    };
    let Some(child_tn_lst) = direct_child_span(&slide_xml, main_seq, "childTnLst")? else {
        return Err(CliError::invalid_args("main sequence has no child steps"));
    };
    let children = direct_children(&slide_xml, child_tn_lst)?;
    let mut steps = BTreeMap::<i64, XmlSpan>::new();
    let mut existing = Vec::new();
    let mut unknown = Vec::new();
    for child in children {
        let span = XmlSpan {
            start: child.start,
            end: child.end,
        };
        if child.kind == "par" {
            if let Some(id) = direct_child_span(&slide_xml, span, "cTn")?
                .and_then(|ctn| element_attr_i64(&slide_xml[ctn.start..ctn.end], "id"))
            {
                steps.insert(id, span);
                existing.push(id);
            }
        } else {
            unknown.push(span);
        }
    }
    validate_step_permutation(&order, &existing)?;
    let (content_start, content_end) =
        element_content_bounds(&slide_xml[child_tn_lst.start..child_tn_lst.end])?;
    let mut replacement = String::new();
    for id in &order {
        let span = steps
            .get(id)
            .ok_or_else(|| CliError::unexpected(format!("missing click step id {id}")))?;
        replacement.push_str(&slide_xml[span.start..span.end]);
    }
    for span in unknown {
        replacement.push_str(&slide_xml[span.start..span.end]);
    }
    let updated_xml = replace_xml_span(
        &slide_xml,
        child_tn_lst.start + content_start,
        child_tn_lst.start + content_end,
        &replacement,
    );
    Ok(ReorderAnimationMutation {
        slide: slide_ref,
        updated_xml,
        click_step_count: order.len(),
        order,
    })
}

fn build_prune_animation_mutation(
    file: &str,
    slide_filter: u32,
    dry_run: bool,
) -> CliResult<PruneAnimationMutation> {
    let refs = pptx_slide_refs_resolved(file)?;
    if slide_filter > refs.len() as u32 {
        return Err(CliError::target_not_found(format!(
            "slide {slide_filter} not found (presentation has {} slides)",
            refs.len()
        )));
    }
    let mut overrides = BTreeMap::new();
    let mut pruned = Vec::new();
    for slide in refs {
        if slide_filter > 0 && slide.number != slide_filter {
            continue;
        }
        let slide_xml = zip_text(file, &slide.part)?;
        let Some(timing) = find_first_element_span(&slide_xml, "timing")? else {
            continue;
        };
        let shape_index =
            build_shape_index(&slide_xml, find_first_element_span(&slide_xml, "spTree")?)?;
        let mut effect_ids = BTreeMap::<i64, String>::new();
        for effect in animation_effects_for_slide(file, slide.number)? {
            if effect.stale && effect.supported && effect.effect_id != 0 {
                effect_ids.insert(effect.effect_id, effect.stale_reason.clone());
                pruned.push(PrunedNode {
                    slide: slide.number,
                    kind: "effect".to_string(),
                    effect_id: effect.effect_id,
                    spid: 0,
                    stale_reason: effect.stale_reason,
                });
            }
        }
        let stale_builds = stale_build_spids(&slide_xml, timing, &shape_index)?;
        for (spid, reason) in &stale_builds {
            pruned.push(PrunedNode {
                slide: slide.number,
                kind: "build".to_string(),
                effect_id: 0,
                spid: *spid,
                stale_reason: reason.clone(),
            });
        }
        if dry_run || (effect_ids.is_empty() && stale_builds.is_empty()) {
            continue;
        }
        let mut updated = slide_xml.clone();
        for id in effect_ids.keys().copied().collect::<Vec<_>>() {
            if let Some(next) = prune_effect_by_id(&updated, id)? {
                updated = next;
            }
        }
        for (spid, _) in stale_builds {
            updated = prune_build_by_spid(&updated, spid)?;
        }
        overrides.insert(slide.part.clone(), updated);
    }
    pruned.sort_by(|left, right| {
        left.slide
            .cmp(&right.slide)
            .then(left.kind.cmp(&right.kind))
            .then(left.effect_id.cmp(&right.effect_id))
            .then(left.spid.cmp(&right.spid))
    });
    Ok(PruneAnimationMutation { overrides, pruned })
}

fn resolve_animation_target(
    file: &str,
    slide: i64,
    shape_selector: &str,
) -> CliResult<ResolvedAnimationTarget> {
    let (slide_ref, shape_selector) =
        if let Some((slide_id, shape_id)) = parse_shape_handle(shape_selector) {
            let refs = pptx_slide_refs_resolved(file)?;
            let matches = refs
                .into_iter()
                .filter(|slide| slide.slide_id == slide_id)
                .collect::<Vec<_>>();
            let slide_ref = match matches.as_slice() {
                [one] => one.clone(),
                [] => {
                    return Err(CliError::target_not_found(format!(
                        "shape handle is stale: slide sldId {slide_id} not found"
                    )));
                }
                _ => {
                    return Err(CliError::target_not_found(format!(
                        "shape handle is ambiguous: slide sldId {slide_id} is duplicated"
                    )));
                }
            };
            (slide_ref, format!("shape:{shape_id}"))
        } else {
            if slide < 1 {
                return Err(CliError::invalid_args("--slide must be >= 1"));
            }
            (
                pptx_slide_ref(file, slide as u32)?,
                shape_selector.to_string(),
            )
        };
    let slide_xml = zip_text(file, &slide_ref.part)?;
    let sp_tree = find_first_element_span(&slide_xml, "spTree")?
        .ok_or_else(|| CliError::unexpected("shape tree not found in slide"))?;
    let shapes = collect_shape_targets(&slide_xml, sp_tree)?;
    let shape = select_shape_target(&shapes, &shape_selector).ok_or_else(|| {
        CliError::target_not_found(format!("shape {shape_selector} not found on slide"))
    })?;
    Ok(ResolvedAnimationTarget {
        slide: slide_ref,
        shape,
    })
}

fn select_shape_target(shapes: &[ShapeTarget], selector: &str) -> Option<ShapeTarget> {
    if let Some(id_text) = selector.strip_prefix("shape:") {
        let id = id_text.trim().parse::<i64>().ok()?;
        return shapes.iter().find(|shape| shape.id == id).cloned();
    }
    if let Some(name) = selector.strip_prefix('~') {
        return shapes.iter().find(|shape| shape.name == name).cloned();
    }
    None
}

fn parse_shape_handle(selector: &str) -> Option<(u32, i64)> {
    let rest = selector.strip_prefix("H:pptx/s:")?;
    let (slide, shape) = rest.split_once("/shape:n:")?;
    Some((slide.parse::<u32>().ok()?, shape.parse::<i64>().ok()?))
}

fn append_effect_pars_to_existing_timing(
    xml: &str,
    timing: XmlSpan,
    effect_pars: &str,
) -> CliResult<String> {
    let Some(main_seq) = find_main_seq_for_mutation(xml, timing)? else {
        return Err(CliError::invalid_args(
            "no main sequence found to append animations",
        ));
    };
    if let Some(child_tn_lst) = direct_child_span(xml, main_seq, "childTnLst")? {
        let (_, content_end) = element_content_bounds(&xml[child_tn_lst.start..child_tn_lst.end])?;
        return Ok(insert_xml_at(
            xml,
            child_tn_lst.start + content_end,
            effect_pars,
        ));
    }
    let insert_at = xml[main_seq.start..main_seq.end]
        .rfind("</")
        .map(|offset| main_seq.start + offset)
        .ok_or_else(|| CliError::unexpected("invalid PPTX timing XML"))?;
    Ok(insert_xml_at(
        xml,
        insert_at,
        &format!("<p:childTnLst>{effect_pars}</p:childTnLst>"),
    ))
}

fn insert_created_timing(xml: &str, effect_pars: &str) -> CliResult<String> {
    let timing = format!(
        concat!(
            r#"<p:timing><p:tnLst><p:par><p:cTn id="1" dur="indefinite" restart="never" nodeType="tmRoot">"#,
            r#"<p:childTnLst><p:seq concurrent="1" nextAc="seek"><p:cTn id="2" dur="indefinite" nodeType="mainSeq">"#,
            r#"<p:childTnLst>{}</p:childTnLst></p:cTn>"#,
            r#"<p:prevCondLst><p:cond evt="onPrev" delay="0"><p:tgtEl><p:sldTgt/></p:tgtEl></p:cond></p:prevCondLst>"#,
            r#"<p:nextCondLst><p:cond evt="onNext" delay="0"><p:tgtEl><p:sldTgt/></p:tgtEl></p:cond></p:nextCondLst>"#,
            r#"</p:seq></p:childTnLst></p:cTn></p:par></p:tnLst></p:timing>"#
        ),
        effect_pars
    );
    let root = find_first_element_span(xml, "sld")?
        .ok_or_else(|| CliError::unexpected("slide root not found"))?;
    let children = direct_children(xml, root)?;
    let insert_at = children
        .iter()
        .find(|child| slide_root_child_rank(&child.kind) > slide_root_child_rank("timing"))
        .map(|child| child.start)
        .unwrap_or_else(|| {
            let (_, content_end) = element_content_bounds(&xml[root.start..root.end])
                .unwrap_or((root.end - root.start, root.end - root.start));
            root.start + content_end
        });
    Ok(insert_xml_at(xml, insert_at, &timing))
}

fn ensure_build_by_paragraph(xml: &str, spid: i64) -> CliResult<String> {
    let timing = find_first_element_span(xml, "timing")?
        .ok_or_else(|| CliError::unexpected("timing missing after animation add"))?;
    let bld_xml = format!(r#"<p:bldP spid="{spid}" grpId="0" build="p"/>"#);
    if let Some(bld_lst) = direct_child_span(xml, timing, "bldLst")? {
        let children = direct_children(xml, bld_lst)?;
        for child in &children {
            if child.kind != "bldP" {
                continue;
            }
            let fragment = &xml[child.start..child.end];
            if element_attr_i64(fragment, "spid") == Some(spid) {
                let updated = replace_or_insert_attr(fragment, "build", "p")?;
                return Ok(replace_xml_span(xml, child.start, child.end, &updated));
            }
        }
        let (_, content_end) = element_content_bounds(&xml[bld_lst.start..bld_lst.end])?;
        return Ok(insert_xml_at(xml, bld_lst.start + content_end, &bld_xml));
    }
    let bld_lst_xml = format!("<p:bldLst>{bld_xml}</p:bldLst>");
    if let Some(tn_lst) = direct_child_span(xml, timing, "tnLst")? {
        return Ok(insert_xml_at(xml, tn_lst.end, &bld_lst_xml));
    }
    let (content_start, _) = element_content_bounds(&xml[timing.start..timing.end])?;
    Ok(insert_xml_at(
        xml,
        timing.start + content_start,
        &bld_lst_xml,
    ))
}

fn find_effect_for_mutation(
    xml: &str,
    timing: XmlSpan,
    effect_id: i64,
    shape_index: &crate::pptx_readback::animations::ShapeIndex,
) -> CliResult<(XmlSpan, XmlSpan, AnimationEffectInfo)> {
    for step in click_step_spans(xml, timing)? {
        let fragment = &xml[step.span.start..step.span.end];
        for ctn in find_descendant_element_spans(fragment, "cTn")? {
            let ctn_abs = XmlSpan {
                start: step.span.start + ctn.start,
                end: step.span.start + ctn.end,
            };
            let ctn_fragment = &xml[ctn_abs.start..ctn_abs.end];
            if element_attr_i64(ctn_fragment, "id") != Some(effect_id) {
                continue;
            }
            let info = classify_effect(ctn_fragment, shape_index);
            let remove_span = innermost_parent_par_span(xml, step.span, ctn_abs)?;
            return Ok((ctn_abs, remove_span, info));
        }
    }
    Err(CliError::target_not_found(format!(
        "effect id {effect_id} not found"
    )))
}

fn innermost_parent_par_span(xml: &str, outer: XmlSpan, child: XmlSpan) -> CliResult<XmlSpan> {
    let mut candidate = outer;
    for par in find_descendant_element_spans(&xml[outer.start..outer.end], "par")? {
        let par = XmlSpan {
            start: outer.start + par.start,
            end: outer.start + par.end,
        };
        if par.start <= child.start
            && child.end <= par.end
            && par.start >= candidate.start
            && par.end <= candidate.end
        {
            candidate = par;
        }
    }
    Ok(candidate)
}

fn find_main_seq_for_mutation(xml: &str, timing: XmlSpan) -> CliResult<Option<XmlSpan>> {
    let timing_fragment = &xml[timing.start..timing.end];
    for ctn in find_descendant_element_spans(timing_fragment, "cTn")? {
        let ctn_fragment = &timing_fragment[ctn.start..ctn.end];
        if crate::pptx_readback::animations::element_attr(ctn_fragment, "nodeType").as_deref()
            == Some("mainSeq")
        {
            return Ok(Some(XmlSpan {
                start: timing.start + ctn.start,
                end: timing.start + ctn.end,
            }));
        }
    }
    Ok(None)
}

fn prune_effect_by_id(xml: &str, effect_id: i64) -> CliResult<Option<String>> {
    let Some(timing) = find_first_element_span(xml, "timing")? else {
        return Ok(None);
    };
    let shape_index = build_shape_index(xml, find_first_element_span(xml, "spTree")?)?;
    let Ok((_, remove_span, info)) = find_effect_for_mutation(xml, timing, effect_id, &shape_index)
    else {
        return Ok(None);
    };
    if !info.supported {
        return Ok(None);
    }
    remove_empty_child_tn_lsts(&remove_xml_span(xml, remove_span.start, remove_span.end)).map(Some)
}

fn remove_empty_child_tn_lsts(xml: &str) -> CliResult<String> {
    let mut out = xml.to_string();
    loop {
        let mut removed_any = false;
        let spans = find_descendant_element_spans(&out, "childTnLst")?;
        for span in spans.into_iter().rev() {
            if direct_children(&out, span)?.is_empty() {
                out = remove_xml_span(&out, span.start, span.end);
                removed_any = true;
            }
        }
        if !removed_any {
            return Ok(out);
        }
    }
}

fn stale_build_spids(
    xml: &str,
    timing: XmlSpan,
    shape_index: &crate::pptx_readback::animations::ShapeIndex,
) -> CliResult<Vec<(i64, String)>> {
    let Some(bld_lst) = direct_child_span(xml, timing, "bldLst")? else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for child in direct_children(xml, bld_lst)? {
        if child.kind != "bldP" {
            continue;
        }
        let fragment = &xml[child.start..child.end];
        let spid = element_attr_i64(fragment, "spid").unwrap_or_default();
        if spid != 0 && !shape_index.names.contains_key(&spid) {
            out.push((spid, "missing-shape".to_string()));
        }
    }
    Ok(out)
}

fn prune_build_by_spid(xml: &str, spid: i64) -> CliResult<String> {
    let Some(timing) = find_first_element_span(xml, "timing")? else {
        return Ok(xml.to_string());
    };
    let Some(bld_lst) = direct_child_span(xml, timing, "bldLst")? else {
        return Ok(xml.to_string());
    };
    for child in direct_children(xml, bld_lst)? {
        if child.kind == "bldP"
            && element_attr_i64(&xml[child.start..child.end], "spid") == Some(spid)
        {
            return Ok(remove_xml_span(xml, child.start, child.end));
        }
    }
    Ok(xml.to_string())
}

fn build_effect_ctn(spec: &EffectBuildSpec<'_>) -> String {
    let (preset_id, preset_subtype) = preset_for_effect(spec.effect);
    let mut child = String::new();
    match spec.effect {
        "appear" => child.push_str(&visibility_set_xml(
            spec.behavior_base_id,
            spec.spid,
            spec.paragraph_range,
        )),
        "fade" => child.push_str(&anim_effect_xml(
            "fade",
            spec.duration_ms,
            spec.behavior_base_id,
            spec.spid,
            spec.paragraph_range,
        )),
        "wipe" => {
            child.push_str(&visibility_set_xml(
                spec.behavior_base_id,
                spec.spid,
                spec.paragraph_range,
            ));
            child.push_str(&anim_effect_xml(
                wipe_filter(spec.direction),
                spec.duration_ms,
                spec.behavior_base_id + 1,
                spec.spid,
                spec.paragraph_range,
            ));
        }
        "flyIn" => {
            child.push_str(&visibility_set_xml(
                spec.behavior_base_id,
                spec.spid,
                spec.paragraph_range,
            ));
            child.push_str(&fly_in_anim_xml(
                spec.direction,
                spec.duration_ms,
                spec.behavior_base_id + 1,
                spec.spid,
                spec.paragraph_range,
            ));
        }
        _ => {}
    }
    format!(
        r#"<p:cTn id="{effect_id}" presetID="{preset_id}" presetClass="entr" presetSubtype="{preset_subtype}" fill="hold" grpId="0" nodeType="{}"><p:stCondLst><p:cond delay="0"/></p:stCondLst><p:childTnLst>{child}</p:childTnLst></p:cTn>"#,
        node_type_for_start(spec.start),
        effect_id = spec.effect_id,
    )
}

fn wrap_effect_par(effect_ctn: &str) -> String {
    format!("<p:par>{effect_ctn}</p:par>")
}

fn visibility_set_xml(c_tn_id: i64, spid: i64, paragraph_range: Option<&ParagraphRange>) -> String {
    format!(
        r#"<p:set>{}<p:to><p:strVal val="visible"/></p:to></p:set>"#,
        c_bhvr_xml(
            c_tn_id,
            "1",
            spid,
            paragraph_range,
            Some("style.visibility"),
            false
        )
    )
}

fn anim_effect_xml(
    filter: &str,
    duration_ms: i64,
    c_tn_id: i64,
    spid: i64,
    paragraph_range: Option<&ParagraphRange>,
) -> String {
    format!(
        r#"<p:animEffect transition="in" filter="{}">{}</p:animEffect>"#,
        xml_attr_escape(filter),
        c_bhvr_xml(
            c_tn_id,
            &duration_ms.to_string(),
            spid,
            paragraph_range,
            None,
            false
        )
    )
}

fn fly_in_anim_xml(
    direction: &str,
    duration_ms: i64,
    c_tn_id: i64,
    spid: i64,
    paragraph_range: Option<&ParagraphRange>,
) -> String {
    let (attr_name, from, to) = fly_in_motion(direction);
    format!(
        concat!(
            r#"<p:anim calcmode="lin" valueType="num">{}"#,
            r#"<p:tavLst><p:tav tm="0"><p:val><p:strVal val="{}"/></p:val></p:tav>"#,
            r#"<p:tav tm="100000"><p:val><p:strVal val="{}"/></p:val></p:tav></p:tavLst></p:anim>"#
        ),
        c_bhvr_xml(
            c_tn_id,
            &duration_ms.to_string(),
            spid,
            paragraph_range,
            Some(attr_name),
            true,
        ),
        xml_attr_escape(from),
        xml_attr_escape(to)
    )
}

fn c_bhvr_xml(
    c_tn_id: i64,
    dur: &str,
    spid: i64,
    paragraph_range: Option<&ParagraphRange>,
    attr_name: Option<&str>,
    additive_base: bool,
) -> String {
    let additive = if additive_base {
        r#" additive="base""#
    } else {
        ""
    };
    let fill = if attr_name == Some("style.visibility") {
        r#" fill="hold""#
    } else {
        ""
    };
    let attr_xml = attr_name.map_or_else(String::new, |name| {
        format!(
            "<p:attrNameLst><p:attrName>{}</p:attrName></p:attrNameLst>",
            xml_escape(name)
        )
    });
    format!(
        r#"<p:cBhvr{additive}><p:cTn id="{c_tn_id}" dur="{}"{fill}/>{}{attr_xml}</p:cBhvr>"#,
        xml_attr_escape(dur),
        sp_tgt_xml(spid, paragraph_range)
    )
}

fn sp_tgt_xml(spid: i64, paragraph_range: Option<&ParagraphRange>) -> String {
    let tx_el = paragraph_range.map_or_else(String::new, |range| {
        format!(
            r#"<p:txEl><p:pRg st="{}" end="{}"/></p:txEl>"#,
            range.start, range.end
        )
    });
    format!(r#"<p:tgtEl><p:spTgt spid="{spid}">{tx_el}</p:spTgt></p:tgtEl>"#)
}

fn max_timing_node_id(timing_fragment: &str) -> i64 {
    find_descendant_element_spans(timing_fragment, "cTn")
        .unwrap_or_default()
        .into_iter()
        .filter_map(|span| element_attr_i64(&timing_fragment[span.start..span.end], "id"))
        .max()
        .unwrap_or_default()
}

fn slide_root_child_rank(local: &str) -> usize {
    match local {
        "cSld" => 0,
        "clrMapOvr" => 1,
        "transition" => 2,
        "timing" => 3,
        "extLst" => 4,
        _ => 5,
    }
}

fn replace_or_insert_attr(fragment: &str, attr_name: &str, value: &str) -> CliResult<String> {
    let open_end = fragment
        .find('>')
        .ok_or_else(|| CliError::unexpected("invalid PPTX XML"))?;
    let open_tag = &fragment[..open_end];
    let pattern = format!("{attr_name}=\"");
    if let Some(start) = open_tag.find(&pattern) {
        let value_start = start + pattern.len();
        let value_end = open_tag[value_start..]
            .find('"')
            .map(|offset| value_start + offset)
            .ok_or_else(|| CliError::unexpected("invalid PPTX XML attribute"))?;
        let mut out = String::new();
        out.push_str(&fragment[..value_start]);
        out.push_str(&xml_attr_escape(value));
        out.push_str(&fragment[value_end..]);
        return Ok(out);
    }
    let insert_at = if open_tag.trim_end().ends_with('/') {
        fragment[..open_end]
            .rfind('/')
            .ok_or_else(|| CliError::unexpected("invalid PPTX XML"))?
    } else {
        open_end
    };
    Ok(insert_xml_at(
        fragment,
        insert_at,
        &format!(r#" {attr_name}="{}""#, xml_attr_escape(value)),
    ))
}

fn insert_xml_at(xml: &str, index: usize, insert: &str) -> String {
    let mut out = String::with_capacity(xml.len() + insert.len());
    out.push_str(&xml[..index]);
    out.push_str(insert);
    out.push_str(&xml[index..]);
    out
}

fn add_animation_result(
    file: &str,
    mutation: &AddAnimationMutation,
    output_path: Option<&str>,
    dry_run: bool,
) -> Value {
    let mut out = Map::new();
    out.insert("file".to_string(), json!(file));
    if let Some(output_path) = output_path {
        out.insert("output".to_string(), json!(output_path));
    }
    out.insert("dryRun".to_string(), json!(dry_run));
    out.insert("action".to_string(), json!("pptx.animations.add"));
    out.insert("slide".to_string(), json!(mutation.slide.number));
    out.insert("shapeId".to_string(), json!(mutation.shape_id));
    out.insert("shapeName".to_string(), json!(mutation.shape_name));
    out.insert("effect".to_string(), json!(mutation.effect));
    out.insert("start".to_string(), json!(mutation.start));
    out.insert(
        "addedEffectIds".to_string(),
        json!(mutation.added_effect_ids),
    );
    out.insert("clickStepId".to_string(), json!(mutation.click_step_id));
    out.insert("createdTiming".to_string(), json!(mutation.created_timing));
    out.insert("byParagraph".to_string(), json!(mutation.by_paragraph));
    if mutation.paragraph_count > 0 {
        out.insert(
            "paragraphCount".to_string(),
            json!(mutation.paragraph_count),
        );
    }
    out.insert(
        "renderUnconfirmed".to_string(),
        json!(matches!(mutation.effect.as_str(), "fade" | "flyIn")),
    );
    add_animation_readback_commands(&mut out, output_path);
    Value::Object(out)
}

fn remove_animation_result(
    file: &str,
    mutation: &RemoveAnimationMutation,
    output_path: Option<&str>,
    dry_run: bool,
) -> Value {
    let mut out = Map::new();
    out.insert("file".to_string(), json!(file));
    if let Some(output_path) = output_path {
        out.insert("output".to_string(), json!(output_path));
    }
    out.insert("dryRun".to_string(), json!(dry_run));
    out.insert("action".to_string(), json!("pptx.animations.remove"));
    out.insert("slide".to_string(), json!(mutation.slide.number));
    out.insert(
        "removedEffectId".to_string(),
        json!(mutation.removed_effect_id),
    );
    out.insert(
        "removedClickStep".to_string(),
        json!(mutation.removed_click_step),
    );
    out.insert("shapeId".to_string(), json!(mutation.shape_id));
    out.insert("shapeName".to_string(), json!(mutation.shape_name));
    add_animation_readback_commands(&mut out, output_path);
    Value::Object(out)
}

fn reorder_animation_result(
    file: &str,
    mutation: &ReorderAnimationMutation,
    output_path: Option<&str>,
    dry_run: bool,
) -> Value {
    let mut out = Map::new();
    out.insert("file".to_string(), json!(file));
    if let Some(output_path) = output_path {
        out.insert("output".to_string(), json!(output_path));
    }
    out.insert("dryRun".to_string(), json!(dry_run));
    out.insert("action".to_string(), json!("pptx.animations.reorder"));
    out.insert("slide".to_string(), json!(mutation.slide.number));
    out.insert("order".to_string(), json!(mutation.order));
    out.insert(
        "clickStepCount".to_string(),
        json!(mutation.click_step_count),
    );
    add_animation_readback_commands(&mut out, output_path);
    Value::Object(out)
}

fn prune_animation_result(
    file: &str,
    slide: u32,
    mutation: &PruneAnimationMutation,
    output_path: Option<&str>,
    dry_run: bool,
) -> Value {
    let mut out = Map::new();
    out.insert("file".to_string(), json!(file));
    if let Some(output_path) = output_path {
        out.insert("output".to_string(), json!(output_path));
    }
    out.insert("dryRun".to_string(), json!(dry_run));
    out.insert("action".to_string(), json!("pptx.animations.prune-stale"));
    out.insert("slide".to_string(), json!(slide));
    out.insert(
        "pruned".to_string(),
        Value::Array(mutation.pruned.iter().map(pruned_node_json).collect()),
    );
    out.insert("prunedCount".to_string(), json!(mutation.pruned.len()));
    add_animation_readback_commands(&mut out, output_path);
    Value::Object(out)
}

fn pruned_node_json(node: &PrunedNode) -> Value {
    let mut out = Map::new();
    out.insert("slide".to_string(), json!(node.slide));
    out.insert("kind".to_string(), json!(node.kind));
    if node.effect_id != 0 {
        out.insert("effectId".to_string(), json!(node.effect_id));
    }
    out.insert("spid".to_string(), json!(node.spid));
    out.insert("staleReason".to_string(), json!(node.stale_reason));
    Value::Object(out)
}

fn add_animation_readback_commands(out: &mut Map<String, Value>, output_path: Option<&str>) {
    let command_target = output_path.unwrap_or("<out.pptx>");
    let suffix = if output_path.is_some() {
        ""
    } else {
        "Template"
    };
    out.insert(
        format!("readbackCommand{suffix}"),
        json!(format!(
            "ooxml --json pptx animations list {}",
            command_arg(command_target)
        )),
    );
    out.insert(
        format!("validateCommand{suffix}"),
        json!(format!(
            "ooxml validate --strict {}",
            command_arg(command_target)
        )),
    );
    out.insert(
        format!("renderCommand{suffix}"),
        json!(format!(
            "ooxml pptx render {} --out render-check",
            command_arg(command_target)
        )),
    );
}

fn animation_effect_not_found_error(file: &str, slide: u32, effect_id: i64) -> CliError {
    let selector = format!("effect:{effect_id}");
    let candidates = animation_effects_for_slide(file, slide)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|effect| {
            if effect.effect_id > 0 {
                Some(format!("effect:{}", effect.effect_id))
            } else {
                None
            }
        })
        .take(3)
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return CliError::target_not_found(format!(
            "animation effect not found: {selector}; discover with `ooxml --json pptx animations list <file>`"
        ));
    }
    CliError::target_not_found(format!(
        "animation effect not found: {selector}; did you mean: {}; discover with `ooxml --json pptx animations list <file>`",
        candidates.join(", ")
    ))
}

fn parse_animation_mutation_options(args: &[String]) -> CliResult<PptxAnimationMutationOptions> {
    let out = parse_string_flag(args, "--out")?;
    let backup = parse_string_flag(args, "--backup")?;
    let dry_run = has_flag(args, "--dry-run");
    let in_place = has_flag(args, "--in-place");
    let no_validate = has_flag(args, "--no-validate");
    validate_xlsx_mutation_output_flags(out.as_deref(), in_place, backup.as_deref(), dry_run)?;
    Ok(PptxAnimationMutationOptions {
        out,
        backup,
        dry_run,
        in_place,
        no_validate,
    })
}

fn animation_mutation_output_path(
    file: &str,
    options: &PptxAnimationMutationOptions,
) -> Option<String> {
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

fn stage_animation_part_mutation(
    file: &str,
    slide_part: &str,
    updated_xml: &str,
    options: &PptxAnimationMutationOptions,
) -> CliResult<String> {
    let output_path = options
        .out
        .as_deref()
        .filter(|value| !value.trim().is_empty());
    let write_path = if options.dry_run || options.in_place || output_path == Some(file) {
        package_mutation_temp_path(file, "pptx-animations")
    } else {
        output_path
            .ok_or_else(|| {
                CliError::invalid_args(
                    "must specify exactly one of --out, --in-place, or --dry-run",
                )
            })?
            .to_string()
    };
    copy_zip_with_part_override(
        file,
        &write_path,
        slide_part.trim_start_matches('/'),
        updated_xml,
    )?;
    if !options.no_validate {
        validate(&write_path, true)?;
    }
    Ok(write_path)
}

fn stage_animation_package_mutation(
    file: &str,
    overrides: &BTreeMap<String, String>,
    options: &PptxAnimationMutationOptions,
) -> CliResult<String> {
    let output_path = options
        .out
        .as_deref()
        .filter(|value| !value.trim().is_empty());
    let write_path = if options.dry_run || options.in_place || output_path == Some(file) {
        package_mutation_temp_path(file, "pptx-animations")
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
    Ok(write_path)
}

fn finish_animation_mutation(
    file: &str,
    staged_path: &str,
    options: &PptxAnimationMutationOptions,
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

fn ensure_pptx(file: &str) -> CliResult<()> {
    let detected = package_type(file)?;
    if detected != "pptx" {
        return Err(CliError::unsupported_type(format!(
            "file is not a PPTX document (detected: {detected})"
        )));
    }
    Ok(())
}

fn parse_paragraph_range(value: &str) -> CliResult<ParagraphRange> {
    let Some((start, end)) = value.split_once(':') else {
        return Err(CliError::invalid_args(
            "--paragraph-range must be in the form A:B (0-based inclusive)",
        ));
    };
    let start = start.trim().parse::<i64>().map_err(|_| {
        CliError::invalid_args("--paragraph-range start must be a non-negative integer")
    })?;
    if start < 0 {
        return Err(CliError::invalid_args(
            "--paragraph-range start must be a non-negative integer",
        ));
    }
    let end = end
        .trim()
        .parse::<i64>()
        .map_err(|_| CliError::invalid_args("--paragraph-range end must be an integer >= start"))?;
    if end < start {
        return Err(CliError::invalid_args(
            "--paragraph-range end must be an integer >= start",
        ));
    }
    Ok(ParagraphRange { start, end })
}

fn parse_int_list(value: &str) -> CliResult<Vec<i64>> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    trimmed
        .split(',')
        .map(|part| {
            let token = part.trim();
            token
                .parse::<i64>()
                .map_err(|_| CliError::invalid_args(format!("invalid id {token:?} in list")))
        })
        .collect()
}

fn validate_step_permutation(order: &[i64], existing: &[i64]) -> CliResult<()> {
    if order.len() != existing.len() {
        return Err(CliError::invalid_args(format!(
            "--order must list all {} click steps (got {}); valid ids: {}",
            existing.len(),
            order.len(),
            join_ids(existing)
        )));
    }
    let existing_set = existing.iter().copied().collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::new();
    for id in order {
        if !existing_set.contains(id) {
            return Err(CliError::invalid_args(format!(
                "--order contains unknown id {id}; valid ids: {}",
                join_ids(existing)
            )));
        }
        if !seen.insert(*id) {
            return Err(CliError::invalid_args(format!(
                "--order contains duplicate id {id}"
            )));
        }
    }
    Ok(())
}

fn join_ids(ids: &[i64]) -> String {
    ids.iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

fn normalize_effect(value: &str) -> CliResult<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "appear" => Ok("appear".to_string()),
        "fade" => Ok("fade".to_string()),
        "wipe" => Ok("wipe".to_string()),
        "fly-in" | "flyin" => Ok("flyIn".to_string()),
        _ => Err(CliError::invalid_args(format!(
            "unknown effect {value:?} (expected appear|fade|wipe|flyIn)"
        ))),
    }
}

fn normalize_start(value: &str) -> CliResult<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "" | "onclick" => Ok("onClick".to_string()),
        "withprevious" => Ok("withPrevious".to_string()),
        "afterprevious" => Ok("afterPrevious".to_string()),
        _ => Err(CliError::invalid_args(format!(
            "unknown start {value:?} (expected onClick|withPrevious|afterPrevious)"
        ))),
    }
}

fn validate_direction(effect: &str, direction: &str) -> CliResult<()> {
    if !matches!(effect, "wipe" | "flyIn") {
        return Ok(());
    }
    if matches!(direction, "up" | "down" | "left" | "right") {
        Ok(())
    } else {
        Err(CliError::invalid_args(format!(
            "unknown direction {direction:?} (expected up|down|left|right)"
        )))
    }
}

fn preset_for_effect(effect: &str) -> (&'static str, &'static str) {
    match effect {
        "appear" => ("1", "0"),
        "fade" => ("10", "0"),
        "wipe" => ("22", "1"),
        "flyIn" => ("2", "4"),
        _ => ("0", "0"),
    }
}

fn wipe_filter(direction: &str) -> &'static str {
    match direction {
        "down" => "wipe(down)",
        "left" => "wipe(left)",
        "right" => "wipe(right)",
        _ => "wipe(up)",
    }
}

fn fly_in_motion(direction: &str) -> (&'static str, &'static str, &'static str) {
    match direction {
        "down" => ("ppt_y", "0-#ppt_h/2", "#ppt_y"),
        "left" => ("ppt_x", "0-#ppt_w/2", "#ppt_x"),
        "right" => ("ppt_x", "1+#ppt_w/2", "#ppt_x"),
        _ => ("ppt_y", "1+#ppt_h/2", "#ppt_y"),
    }
}

fn node_type_for_start(start: &str) -> &'static str {
    match start {
        "withPrevious" => "withEffect",
        "afterPrevious" => "afterEffect",
        _ => "clickEffect",
    }
}
