use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};

use crate::xml_util::decode_xml_text;
use crate::{
    CliError, CliResult, RelationshipEntry, attr, attr_exact, local_name, relationship_entries,
    relationships, relationships_part_for, resolve_relationship_target, xml_direct_child_ranges,
    zip_entry_names, zip_entry_set, zip_text,
};

const PRG_SENTINEL: i64 = 4_294_967_295;

#[derive(Clone, Copy, Debug)]
pub(crate) struct XmlSpan {
    pub(crate) start: usize,
    pub(crate) end: usize,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ShapeIndex {
    pub(crate) names: BTreeMap<i64, String>,
    pub(crate) para_count: BTreeMap<i64, usize>,
}

#[derive(Clone, Debug)]
pub(crate) struct ShapeTarget {
    pub(crate) id: i64,
    pub(crate) name: String,
    pub(crate) para_count: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct SlideRef {
    pub(crate) number: u32,
    pub(crate) slide_id: u32,
    pub(crate) part: String,
    pub(crate) part_uri: String,
}

#[derive(Clone, Debug)]
pub(crate) struct ParagraphRange {
    pub(crate) start: i64,
    pub(crate) end: i64,
}

#[derive(Clone, Debug)]
struct Behavior {
    local: String,
    fragment: String,
}

#[derive(Clone, Debug)]
pub(crate) struct AnimationEffectInfo {
    pub(crate) sequence_pos: usize,
    pub(crate) click_step: usize,
    pub(crate) effect_id: i64,
    pub(crate) click_step_id: i64,
    pub(crate) effect_kind: String,
    pub(crate) supported: bool,
    pub(crate) preset_class: String,
    pub(crate) preset_id: String,
    pub(crate) preset_subtype: String,
    pub(crate) filter: String,
    pub(crate) start_type: String,
    pub(crate) spid: i64,
    pub(crate) shape_name: String,
    pub(crate) paragraph_range: Option<ParagraphRange>,
    pub(crate) stale: bool,
    pub(crate) stale_reason: String,
}

#[derive(Clone, Debug)]
pub(crate) struct ClickStepSpan {
    pub(crate) span: XmlSpan,
}

pub(crate) fn pptx_animations_list(file: &str) -> CliResult<Value> {
    let slides = pptx_slide_refs_resolved(file)?;
    let mut slide_values = Vec::with_capacity(slides.len());
    for slide in slides {
        let slide_xml = zip_text(file, &slide.part)?;
        slide_values.push(slide_animation_json(file, &slide, &slide_xml)?);
    }
    Ok(json!({ "slides": slide_values }))
}

pub(crate) fn pptx_slide_refs_resolved(file: &str) -> CliResult<Vec<SlideRef>> {
    let presentation = zip_text(file, "ppt/presentation.xml")?;
    let slide_refs = pptx_slide_refs(&presentation);
    let rels = relationships(file, "ppt/_rels/presentation.xml.rels")?;
    slide_refs
        .into_iter()
        .enumerate()
        .map(|(index, (slide_id, rel_id))| {
            let target = rels
                .get(&rel_id)
                .ok_or_else(|| CliError::unexpected(format!("missing relationship {rel_id}")))?;
            let part = normalize_ppt_target(target);
            Ok(SlideRef {
                number: index as u32 + 1,
                slide_id,
                part_uri: format!("/{part}"),
                part,
            })
        })
        .collect()
}

pub(crate) fn pptx_slide_ref(file: &str, slide: u32) -> CliResult<SlideRef> {
    if slide == 0 {
        return Err(CliError::invalid_args("--slide must be >= 1"));
    }
    let refs = pptx_slide_refs_resolved(file)?;
    refs.get(slide as usize - 1).cloned().ok_or_else(|| {
        CliError::target_not_found(format!(
            "slide {slide} not found (presentation has {} slides)",
            refs.len()
        ))
    })
}

fn slide_animation_json(file: &str, slide: &SlideRef, slide_xml: &str) -> CliResult<Value> {
    let sp_tree = find_first_element_span(slide_xml, "spTree")?;
    let shape_index = build_shape_index(slide_xml, sp_tree)?;
    let timing = find_first_element_span(slide_xml, "timing")?;
    let media = collect_media(
        file,
        &slide.part_uri,
        slide_xml,
        sp_tree,
        timing,
        &shape_index,
    )?;

    let mut out = Map::new();
    out.insert("slide".to_string(), json!(slide.number));
    out.insert("partUri".to_string(), json!(slide.part_uri));
    out.insert("hasTiming".to_string(), json!(timing.is_some()));
    if let Some(timing) = timing {
        let (effects, unsupported_count) = collect_effects(slide_xml, timing, &shape_index)?;
        let builds = collect_builds(slide_xml, timing, &shape_index)?;
        out.insert(
            "effects".to_string(),
            Value::Array(effects.iter().map(effect_json).collect()),
        );
        if !builds.is_empty() {
            out.insert("builds".to_string(), Value::Array(builds));
        }
        if !media.is_empty() {
            out.insert("media".to_string(), Value::Array(media));
        }
        out.insert("unsupportedCount".to_string(), json!(unsupported_count));
    } else {
        out.insert("effects".to_string(), Value::Array(Vec::new()));
        if !media.is_empty() {
            out.insert("media".to_string(), Value::Array(media));
        }
        out.insert("unsupportedCount".to_string(), json!(0));
    }
    Ok(Value::Object(out))
}

pub(crate) fn animation_effects_for_slide(
    file: &str,
    slide: u32,
) -> CliResult<Vec<AnimationEffectInfo>> {
    let slide_ref = pptx_slide_ref(file, slide)?;
    let slide_xml = zip_text(file, &slide_ref.part)?;
    let Some(timing) = find_first_element_span(&slide_xml, "timing")? else {
        return Ok(Vec::new());
    };
    let shape_index =
        build_shape_index(&slide_xml, find_first_element_span(&slide_xml, "spTree")?)?;
    collect_effects(&slide_xml, timing, &shape_index).map(|(effects, _)| effects)
}

pub(crate) fn click_step_spans(xml: &str, timing: XmlSpan) -> CliResult<Vec<ClickStepSpan>> {
    let Some(main_seq) = find_main_seq_ctn(xml, timing)? else {
        return Ok(Vec::new());
    };
    let Some(child_tn_lst) = direct_child_span(xml, main_seq, "childTnLst")? else {
        return Ok(Vec::new());
    };
    let mut steps = Vec::new();
    for child in direct_children(xml, child_tn_lst)? {
        if child.kind != "par" {
            continue;
        }
        steps.push(ClickStepSpan {
            span: XmlSpan {
                start: child.start,
                end: child.end,
            },
        });
    }
    Ok(steps)
}

fn effect_json(effect: &AnimationEffectInfo) -> Value {
    let mut out = Map::new();
    out.insert("sequencePos".to_string(), json!(effect.sequence_pos));
    out.insert("clickStep".to_string(), json!(effect.click_step));
    out.insert("effectId".to_string(), json!(effect.effect_id));
    out.insert("clickStepId".to_string(), json!(effect.click_step_id));
    if effect.effect_id > 0 {
        out.insert(
            "primarySelector".to_string(),
            json!(format!("effect:{}", effect.effect_id)),
        );
        let mut selectors = vec![
            format!("effect:{}", effect.effect_id),
            effect.effect_id.to_string(),
        ];
        if effect.click_step_id > 0 {
            selectors.push(format!("clickStep:{}", effect.click_step_id));
        }
        out.insert("selectors".to_string(), json!(selectors));
    }
    out.insert("effectKind".to_string(), json!(effect.effect_kind));
    out.insert("supported".to_string(), json!(effect.supported));
    if !effect.preset_class.is_empty() {
        out.insert("presetClass".to_string(), json!(effect.preset_class));
    }
    if !effect.preset_id.is_empty() {
        out.insert("presetId".to_string(), json!(effect.preset_id));
    }
    if !effect.preset_subtype.is_empty() {
        out.insert("presetSubtype".to_string(), json!(effect.preset_subtype));
    }
    if !effect.filter.is_empty() {
        out.insert("filter".to_string(), json!(effect.filter));
    }
    out.insert("startType".to_string(), json!(effect.start_type));
    out.insert("spid".to_string(), json!(effect.spid));
    if !effect.shape_name.is_empty() {
        out.insert("shapeName".to_string(), json!(effect.shape_name));
    }
    if let Some(range) = effect.paragraph_range.as_ref() {
        out.insert(
            "paragraphRange".to_string(),
            json!({
                "start": range.start,
                "end": range.end,
            }),
        );
    }
    out.insert("stale".to_string(), json!(effect.stale));
    if !effect.stale_reason.is_empty() {
        out.insert("staleReason".to_string(), json!(effect.stale_reason));
    }
    Value::Object(out)
}

fn collect_effects(
    xml: &str,
    timing: XmlSpan,
    shape_index: &ShapeIndex,
) -> CliResult<(Vec<AnimationEffectInfo>, usize)> {
    let Some(main_seq) = find_main_seq_ctn(xml, timing)? else {
        return Ok((Vec::new(), 0));
    };
    let Some(child_tn_lst) = direct_child_span(xml, main_seq, "childTnLst")? else {
        return Ok((Vec::new(), 0));
    };
    let mut effects = Vec::new();
    let mut unsupported_count = 0usize;
    let mut click_step = 0usize;
    for child in direct_children(xml, child_tn_lst)? {
        if child.kind != "par" {
            continue;
        }
        let par_span = XmlSpan {
            start: child.start,
            end: child.end,
        };
        let click_step_id = direct_child_span(xml, par_span, "cTn")?
            .and_then(|span| element_attr_i64(&xml[span.start..span.end], "id"))
            .unwrap_or_default();
        for eff in find_descendant_element_spans(&xml[par_span.start..par_span.end], "cTn")? {
            let eff = XmlSpan {
                start: par_span.start + eff.start,
                end: par_span.start + eff.end,
            };
            if element_attr(&xml[eff.start..eff.end], "presetClass").is_none() {
                continue;
            }
            let mut rec = classify_effect(&xml[eff.start..eff.end], shape_index);
            rec.sequence_pos = effects.len();
            rec.click_step = click_step;
            rec.click_step_id = click_step_id;
            rec.effect_id = element_attr_i64(&xml[eff.start..eff.end], "id").unwrap_or_default();
            if !rec.supported {
                unsupported_count += 1;
            }
            effects.push(rec);
        }
        click_step += 1;
    }
    Ok((effects, unsupported_count))
}

pub(crate) fn classify_effect(fragment: &str, shape_index: &ShapeIndex) -> AnimationEffectInfo {
    let preset_class = element_attr(fragment, "presetClass").unwrap_or_default();
    let preset_id = element_attr(fragment, "presetID").unwrap_or_default();
    let preset_subtype = element_attr(fragment, "presetSubtype").unwrap_or_default();
    let node_type = element_attr(fragment, "nodeType").unwrap_or_default();
    let start_type = start_type_from_node_type(&node_type);
    let behaviors = collect_behaviors(fragment).unwrap_or_default();
    let filter = first_anim_effect_filter(&behaviors);
    let (spid, paragraph_range) = target_from_behaviors(&behaviors);
    let (effect_kind, supported) = classify_kind(&preset_class, &behaviors, &filter);
    let mut rec = AnimationEffectInfo {
        sequence_pos: 0,
        click_step: 0,
        effect_id: 0,
        click_step_id: 0,
        effect_kind,
        supported,
        preset_class,
        preset_id,
        preset_subtype,
        filter,
        start_type,
        spid,
        shape_name: String::new(),
        paragraph_range,
        stale: false,
        stale_reason: String::new(),
    };
    apply_stale(&mut rec, shape_index);
    rec
}

fn collect_behaviors(fragment: &str) -> CliResult<Vec<Behavior>> {
    let Some(child_tn_lst) = find_first_direct_child_span(fragment, "childTnLst")? else {
        return Ok(Vec::new());
    };
    let mut behaviors = Vec::new();
    for child in direct_children(fragment, child_tn_lst)? {
        if matches!(
            child.kind.as_str(),
            "set"
                | "animEffect"
                | "anim"
                | "cmd"
                | "animMotion"
                | "animClr"
                | "animRot"
                | "animScale"
                | "audio"
                | "video"
        ) {
            behaviors.push(Behavior {
                local: child.kind,
                fragment: fragment[child.start..child.end].to_string(),
            });
        }
    }
    Ok(behaviors)
}

fn classify_kind(preset_class: &str, behaviors: &[Behavior], filter: &str) -> (String, bool) {
    if preset_class == "entr" {
        if only_visibility_set(behaviors) {
            return ("appear".to_string(), true);
        }
        if has_in_filter(behaviors, "fade") {
            return ("fade".to_string(), true);
        }
        if has_in_filter_prefix(behaviors, "wipe") {
            return ("wipe".to_string(), true);
        }
        if has_position_anim(behaviors) {
            return ("flyIn".to_string(), true);
        }
    }
    (
        format!(
            "unsupported:{}",
            unsupported_raw(preset_class, behaviors, filter)
        ),
        false,
    )
}

fn only_visibility_set(behaviors: &[Behavior]) -> bool {
    behaviors.len() == 1
        && behaviors[0].local == "set"
        && first_descendant_text(&behaviors[0].fragment, "attrName").as_deref()
            == Some("style.visibility")
}

fn has_in_filter(behaviors: &[Behavior], filter: &str) -> bool {
    behaviors.iter().any(|behavior| {
        behavior.local == "animEffect"
            && element_attr(&behavior.fragment, "transition").as_deref() == Some("in")
            && element_attr(&behavior.fragment, "filter").as_deref() == Some(filter)
    })
}

fn has_in_filter_prefix(behaviors: &[Behavior], prefix: &str) -> bool {
    behaviors.iter().any(|behavior| {
        behavior.local == "animEffect"
            && element_attr(&behavior.fragment, "transition").as_deref() == Some("in")
            && element_attr(&behavior.fragment, "filter")
                .is_some_and(|value| value.starts_with(prefix))
    })
}

fn has_position_anim(behaviors: &[Behavior]) -> bool {
    behaviors.iter().any(|behavior| {
        behavior.local == "anim"
            && matches!(
                first_descendant_text(&behavior.fragment, "attrName").as_deref(),
                Some("ppt_x" | "ppt_y")
            )
    })
}

fn unsupported_raw(preset_class: &str, behaviors: &[Behavior], filter: &str) -> String {
    if !preset_class.is_empty() && preset_class != "entr" {
        if let Some(first) = behaviors.first() {
            return format!("{preset_class}/{}", first.local);
        }
        return preset_class.to_string();
    }
    let Some(first) = behaviors.first() else {
        return "empty".to_string();
    };
    if first.local == "animEffect" && !filter.is_empty() {
        return format!("animEffect({filter})");
    }
    first.local.clone()
}

fn target_from_behaviors(behaviors: &[Behavior]) -> (i64, Option<ParagraphRange>) {
    for behavior in behaviors {
        let Ok(Some(sp_tgt)) = find_first_element_span(&behavior.fragment, "spTgt") else {
            continue;
        };
        let sp_fragment = &behavior.fragment[sp_tgt.start..sp_tgt.end];
        let spid = element_attr_i64(sp_fragment, "spid").unwrap_or_default();
        let paragraph_range = find_first_element_span(sp_fragment, "pRg")
            .ok()
            .flatten()
            .and_then(|p_rg| {
                let pr = &sp_fragment[p_rg.start..p_rg.end];
                Some(ParagraphRange {
                    start: element_attr_i64(pr, "st")?,
                    end: element_attr_i64(pr, "end")?,
                })
            });
        return (spid, paragraph_range);
    }
    (0, None)
}

fn first_anim_effect_filter(behaviors: &[Behavior]) -> String {
    behaviors
        .iter()
        .find(|behavior| behavior.local == "animEffect")
        .and_then(|behavior| element_attr(&behavior.fragment, "filter"))
        .unwrap_or_default()
}

fn apply_stale(effect: &mut AnimationEffectInfo, shape_index: &ShapeIndex) {
    if effect.spid == 0 {
        return;
    }
    let Some(name) = shape_index.names.get(&effect.spid) else {
        effect.stale = true;
        effect.stale_reason = "missing-shape".to_string();
        return;
    };
    effect.shape_name.clone_from(name);
    let Some(range) = effect.paragraph_range.as_ref() else {
        return;
    };
    if range.start == PRG_SENTINEL || range.end == PRG_SENTINEL {
        return;
    }
    let count = shape_index
        .para_count
        .get(&effect.spid)
        .copied()
        .unwrap_or_default();
    if range.start >= count as i64 || range.end >= count as i64 {
        effect.stale = true;
        effect.stale_reason = format!("pRg-out-of-range:{}-{}/{count}", range.start, range.end);
    }
}

fn collect_builds(xml: &str, timing: XmlSpan, shape_index: &ShapeIndex) -> CliResult<Vec<Value>> {
    let Some(bld_lst) = direct_child_span(xml, timing, "bldLst")? else {
        return Ok(Vec::new());
    };
    let mut builds = Vec::new();
    for child in direct_children(xml, bld_lst)? {
        if child.kind != "bldP" {
            continue;
        }
        let fragment = &xml[child.start..child.end];
        let spid = element_attr_i64(fragment, "spid").unwrap_or_default();
        let build = element_attr(fragment, "build").unwrap_or_default();
        let grp_id = element_attr(fragment, "grpId").unwrap_or_default();
        let mut out = Map::new();
        out.insert("spid".to_string(), json!(spid));
        if let Some(shape_name) = shape_index.names.get(&spid) {
            out.insert("shapeName".to_string(), json!(shape_name));
        }
        out.insert("build".to_string(), json!(build));
        if !grp_id.is_empty() {
            out.insert("grpId".to_string(), json!(grp_id));
        }
        let stale = spid != 0 && !shape_index.names.contains_key(&spid);
        out.insert("stale".to_string(), json!(stale));
        if stale {
            out.insert("staleReason".to_string(), json!("missing-shape"));
        }
        builds.push(Value::Object(out));
    }
    Ok(builds)
}

fn collect_media(
    file: &str,
    part_uri: &str,
    xml: &str,
    sp_tree: Option<XmlSpan>,
    timing: Option<XmlSpan>,
    shape_index: &ShapeIndex,
) -> CliResult<Vec<Value>> {
    let Some(sp_tree) = sp_tree else {
        return Ok(Vec::new());
    };
    let rels = relationship_entries(file, &relationships_part_for(part_uri)).unwrap_or_default();
    let rel_map = rels
        .into_iter()
        .map(|rel| (rel.id.clone(), rel))
        .collect::<BTreeMap<_, _>>();
    let part_set = zip_entry_set(&zip_entry_names(file)?);
    let mut media = Vec::new();
    for pic in find_descendant_element_spans(&xml[sp_tree.start..sp_tree.end], "pic")? {
        let pic = XmlSpan {
            start: sp_tree.start + pic.start,
            end: sp_tree.start + pic.end,
        };
        if let Some(value) = media_from_pic(
            &xml[pic.start..pic.end],
            part_uri,
            &rel_map,
            &part_set,
            timing.map(|span| &xml[span.start..span.end]),
            shape_index,
        ) {
            media.push(value);
        }
    }
    Ok(media)
}

fn media_from_pic(
    pic: &str,
    part_uri: &str,
    rel_map: &BTreeMap<String, RelationshipEntry>,
    part_set: &BTreeSet<String>,
    timing: Option<&str>,
    shape_index: &ShapeIndex,
) -> Option<Value> {
    let video_file = find_first_element_span(pic, "videoFile").ok().flatten();
    let audio_file = find_first_element_span(pic, "audioFile").ok().flatten();
    let p14_media = find_first_element_span(pic, "media").ok().flatten();
    if video_file.is_none() && audio_file.is_none() && p14_media.is_none() {
        return None;
    }
    let kind = if video_file.is_some() {
        "video"
    } else if audio_file.is_some() {
        "audio"
    } else {
        "unknown"
    };
    let c_nv_pr = find_first_element_span(pic, "cNvPr").ok().flatten();
    let spid = c_nv_pr
        .and_then(|span| element_attr_i64(&pic[span.start..span.end], "id"))
        .unwrap_or_default();

    let media_rid = p14_media
        .and_then(|span| element_attr(&pic[span.start..span.end], "embed"))
        .or_else(|| video_file.and_then(|span| element_attr(&pic[span.start..span.end], "link")))
        .or_else(|| audio_file.and_then(|span| element_attr(&pic[span.start..span.end], "link")))
        .unwrap_or_default();
    let poster_rid = find_first_element_span(pic, "blip")
        .ok()
        .flatten()
        .and_then(|span| element_attr(&pic[span.start..span.end], "embed"))
        .unwrap_or_default();

    let mut out = Map::new();
    out.insert("spid".to_string(), json!(spid));
    if let Some(name) = shape_index.names.get(&spid) {
        out.insert("shapeName".to_string(), json!(name));
    }
    out.insert("kind".to_string(), json!(kind));

    let mut stale = false;
    let mut stale_reason = String::new();
    let mut external = false;
    if !media_rid.is_empty() {
        let resolved = resolve_media_rel(part_uri, &media_rid, rel_map, part_set);
        if resolved.external {
            external = true;
        }
        if !resolved.uri.is_empty() {
            out.insert("mediaPartUri".to_string(), json!(resolved.uri));
        }
        if !resolved.reason.is_empty() {
            stale = true;
            stale_reason = resolved.reason;
        }
    }
    if !poster_rid.is_empty() {
        let resolved = resolve_media_rel(part_uri, &poster_rid, rel_map, part_set);
        if resolved.external {
            external = true;
        }
        if !resolved.uri.is_empty() {
            out.insert("posterPartUri".to_string(), json!(resolved.uri));
        }
        if !resolved.reason.is_empty() && !stale {
            stale = true;
            stale_reason = resolved.reason;
        }
    }
    out.insert(
        "hasClickToPlay".to_string(),
        json!(timing.is_some_and(|timing| has_click_to_play(timing, spid))),
    );
    if external {
        out.insert("isExternal".to_string(), json!(true));
    }
    out.insert("stale".to_string(), json!(stale));
    if stale {
        out.insert("staleReason".to_string(), json!(stale_reason));
    }
    Some(Value::Object(out))
}

struct ResolvedMediaRel {
    uri: String,
    reason: String,
    external: bool,
}

fn resolve_media_rel(
    part_uri: &str,
    rid: &str,
    rel_map: &BTreeMap<String, RelationshipEntry>,
    part_set: &BTreeSet<String>,
) -> ResolvedMediaRel {
    let Some(rel) = rel_map.get(rid) else {
        return ResolvedMediaRel {
            uri: String::new(),
            reason: format!("dangling-rel:{rid}"),
            external: false,
        };
    };
    if rel.target_mode.eq_ignore_ascii_case("External") {
        return ResolvedMediaRel {
            uri: rel.target.clone(),
            reason: String::new(),
            external: true,
        };
    }
    let target = resolve_relationship_target(part_uri, &rel.target);
    let reason = if part_set.contains(&target) {
        String::new()
    } else {
        format!("missing-part:{target}")
    };
    ResolvedMediaRel {
        uri: target,
        reason,
        external: false,
    }
}

fn has_click_to_play(timing: &str, spid: i64) -> bool {
    if spid == 0 {
        return false;
    }
    let Ok(commands) = find_descendant_element_spans(timing, "cmd") else {
        return false;
    };
    commands.into_iter().any(|span| {
        let fragment = &timing[span.start..span.end];
        element_attr(fragment, "type").as_deref() == Some("call")
            && element_attr(fragment, "cmd").is_some_and(|cmd| cmd.starts_with("playFrom"))
            && find_first_element_span(fragment, "spTgt")
                .ok()
                .flatten()
                .and_then(|sp_tgt| element_attr_i64(&fragment[sp_tgt.start..sp_tgt.end], "spid"))
                == Some(spid)
    })
}

pub(crate) fn build_shape_index(xml: &str, sp_tree: Option<XmlSpan>) -> CliResult<ShapeIndex> {
    let Some(sp_tree) = sp_tree else {
        return Ok(ShapeIndex::default());
    };
    let mut index = ShapeIndex::default();
    let fragment = &xml[sp_tree.start..sp_tree.end];
    for span in find_descendant_element_spans(fragment, "cNvPr")? {
        let c_nv_pr = &fragment[span.start..span.end];
        if let Some(id) = element_attr_i64(c_nv_pr, "id") {
            index
                .names
                .insert(id, element_attr(c_nv_pr, "name").unwrap_or_default());
        }
    }
    for shape in collect_shape_targets(xml, sp_tree)? {
        if shape.para_count > 0 {
            index.para_count.insert(shape.id, shape.para_count);
        }
    }
    Ok(index)
}

pub(crate) fn collect_shape_targets(xml: &str, sp_tree: XmlSpan) -> CliResult<Vec<ShapeTarget>> {
    let mut targets = Vec::new();
    let fragment = &xml[sp_tree.start..sp_tree.end];
    for span in find_element_spans_by_locals(fragment, &["sp", "pic", "graphicFrame", "grpSp"])? {
        let span = XmlSpan {
            start: sp_tree.start + span.start,
            end: sp_tree.start + span.end,
        };
        let shape_xml = &xml[span.start..span.end];
        let Some(c_nv_pr) = find_first_element_span(shape_xml, "cNvPr")? else {
            continue;
        };
        let c_nv_pr_fragment = &shape_xml[c_nv_pr.start..c_nv_pr.end];
        let Some(id) = element_attr_i64(c_nv_pr_fragment, "id") else {
            continue;
        };
        let name = element_attr(c_nv_pr_fragment, "name").unwrap_or_default();
        let para_count = count_direct_paragraphs(shape_xml)?;
        targets.push(ShapeTarget {
            id,
            name,
            para_count,
        });
    }
    Ok(targets)
}

fn count_direct_paragraphs(shape_xml: &str) -> CliResult<usize> {
    let Some(tx_body) = find_first_direct_child_span(shape_xml, "txBody")? else {
        return Ok(0);
    };
    direct_children(shape_xml, tx_body).map(|children| {
        children
            .into_iter()
            .filter(|child| child.kind == "p")
            .count()
    })
}

fn find_main_seq_ctn(xml: &str, timing: XmlSpan) -> CliResult<Option<XmlSpan>> {
    let timing_fragment = &xml[timing.start..timing.end];
    let Some(tn_lst) = find_first_direct_child_span(timing_fragment, "tnLst")? else {
        return Ok(None);
    };
    let tn_fragment = &timing_fragment[tn_lst.start..tn_lst.end];
    for span in find_descendant_element_spans(tn_fragment, "seq")? {
        let seq_fragment = &tn_fragment[span.start..span.end];
        let Some(c_tn) = find_first_direct_child_span(seq_fragment, "cTn")? else {
            continue;
        };
        let c_tn_fragment = &seq_fragment[c_tn.start..c_tn.end];
        if element_attr(c_tn_fragment, "nodeType").as_deref() == Some("mainSeq") {
            return Ok(Some(XmlSpan {
                start: timing.start + tn_lst.start + span.start + c_tn.start,
                end: timing.start + tn_lst.start + span.start + c_tn.end,
            }));
        }
    }
    Ok(None)
}

fn start_type_from_node_type(value: &str) -> String {
    match value {
        "clickEffect" => "onClick",
        "withEffect" => "withPrevious",
        "afterEffect" => "afterPrevious",
        _ => "unknown",
    }
    .to_string()
}

pub(crate) fn find_first_element_span(xml: &str, wanted_local: &str) -> CliResult<Option<XmlSpan>> {
    Ok(find_descendant_element_spans(xml, wanted_local)?
        .into_iter()
        .next())
}

fn find_first_direct_child_span(xml: &str, wanted_local: &str) -> CliResult<Option<XmlSpan>> {
    let whole = XmlSpan {
        start: 0,
        end: xml.len(),
    };
    direct_child_span(xml, whole, wanted_local)
}

pub(crate) fn direct_child_span(
    xml: &str,
    parent: XmlSpan,
    wanted_local: &str,
) -> CliResult<Option<XmlSpan>> {
    Ok(direct_children(xml, parent)?
        .into_iter()
        .find(|child| child.kind == wanted_local)
        .map(|child| XmlSpan {
            start: child.start,
            end: child.end,
        }))
}

pub(crate) fn direct_children(xml: &str, parent: XmlSpan) -> CliResult<Vec<crate::XmlNamedRange>> {
    let (content_start, content_end) = element_content_bounds(&xml[parent.start..parent.end])?;
    xml_direct_child_ranges(
        xml,
        parent.start + content_start,
        parent.start + content_end,
    )
}

pub(crate) fn element_content_bounds(fragment: &str) -> CliResult<(usize, usize)> {
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

pub(crate) fn find_descendant_element_spans(
    xml: &str,
    wanted_local: &str,
) -> CliResult<Vec<XmlSpan>> {
    find_element_spans_by_locals(xml, &[wanted_local])
}

fn find_element_spans_by_locals(xml: &str, wanted_locals: &[&str]) -> CliResult<Vec<XmlSpan>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut stack: Vec<(String, usize)> = Vec::new();
    let mut spans = Vec::new();
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                stack.push((local_name(e.name().as_ref()).to_string(), before));
            }
            Ok(Event::Empty(e)) => {
                if wanted_locals
                    .iter()
                    .any(|wanted| *wanted == local_name(e.name().as_ref()))
                {
                    spans.push(XmlSpan {
                        start: before,
                        end: reader.buffer_position() as usize,
                    });
                }
            }
            Ok(Event::End(_)) => {
                if let Some((local, start)) = stack.pop()
                    && wanted_locals.iter().any(|wanted| *wanted == local)
                {
                    spans.push(XmlSpan {
                        start,
                        end: reader.buffer_position() as usize,
                    });
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    spans.sort_by_key(|span| span.start);
    Ok(spans)
}

pub(crate) fn element_attr(fragment: &str, wanted_local: &str) -> Option<String> {
    let mut reader = Reader::from_str(fragment);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => return attr(&e, wanted_local),
            Ok(Event::Eof) | Err(_) => return None,
            _ => {}
        }
    }
}

pub(crate) fn element_attr_i64(fragment: &str, wanted_local: &str) -> Option<i64> {
    element_attr(fragment, wanted_local).and_then(|value| value.trim().parse::<i64>().ok())
}

fn first_descendant_text(fragment: &str, wanted_local: &str) -> Option<String> {
    let mut reader = Reader::from_str(fragment);
    let mut depth: Option<usize> = None;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == wanted_local => {
                depth = Some(1);
            }
            Ok(Event::Start(_)) => {
                if let Some(value) = depth.as_mut() {
                    *value += 1;
                }
            }
            Ok(Event::Text(e)) if depth.is_some() => return Some(decode_xml_text(e.as_ref())),
            Ok(Event::End(e)) => {
                if let Some(value) = depth.as_mut() {
                    if *value == 1 && local_name(e.name().as_ref()) == wanted_local {
                        depth = None;
                    } else {
                        *value = value.saturating_sub(1);
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => return None,
            _ => {}
        }
    }
}

fn pptx_slide_refs(xml: &str) -> Vec<(u32, String)> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut slides = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "sldId" =>
            {
                if let (Some(id), Some(rel)) = (attr_exact(&e, "id"), attr_exact(&e, "r:id"))
                    && let Ok(id) = id.parse::<u32>()
                {
                    slides.push((id, rel));
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    slides
}

fn normalize_ppt_target(target: &str) -> String {
    let target = target.trim_start_matches('/');
    if target.starts_with("ppt/") {
        target.to_string()
    } else {
        format!("ppt/{}", target.trim_start_matches("../"))
    }
}
