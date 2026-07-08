use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use crate::cli_args::value_flag_present;
use crate::{
    CliError, CliResult, RelationshipEntry, allocate_relationship_id, attr, attr_exact,
    content_type_for_part, copy_zip_with_binary_part_overrides_and_removals,
    ensure_content_type_override, local_name, package_mutation_temp_path, package_type,
    relationship_entries, relationship_target_from_source_to_target, relationships_part_for,
    replace_xml_span, resolve_relationship_target, validate, validate_xlsx_mutation_output_flags,
    xml_attr_escape, zip_entry_exists, zip_entry_names, zip_text,
};

mod media_types;
mod output;

use media_types::{
    content_type_for_media_ext, extension_for_content_type, file_extension_with_dot,
    poster_content_type_for_path, reject_media_url, resolve_media_kind,
};
use output::{media_add_result_json, media_replace_result_json};

const REL_TYPE_MEDIA: &str = "http://schemas.microsoft.com/office/2007/relationships/media";
const REL_TYPE_VIDEO: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/video";
const REL_TYPE_AUDIO: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/audio";
const REL_TYPE_IMAGE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";
const P14_MEDIA_NS: &str = "http://schemas.microsoft.com/office/powerpoint/2010/main";
const MEDIA_EXT_URI: &str = "{DAA4B4D4-6D71-4841-9C94-3DE7FCFB9230}";
const HLINK_MEDIA_ACTION: &str = "ppaction://media";
const PLAY_FROM_CMD: &str = "playFrom(0.0)";
const DEFAULT_MEDIA_VOLUME: i64 = 80;
const EMU_PER_INCH: i64 = 914400;

const POSTER_PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x70, 0x70, 0x70, 0xf8,
    0x0f, 0x00, 0x02, 0x05, 0x01, 0x01, 0x5f, 0x4f, 0x65, 0x8d, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
    0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

#[derive(Clone)]
struct SlideRef {
    number: u32,
    part: String,
}

#[derive(Clone, Copy, Default)]
struct XmlSpan {
    start: usize,
    end: usize,
}

#[derive(Clone, Default)]
struct PicInfo {
    span: XmlSpan,
    shape_id: u32,
    shape_name: String,
    kind: String,
    media_rel_id: String,
    av_rel_id: String,
    poster_rel_id: String,
    has_media_hlink: bool,
    is_media: bool,
}

#[derive(Clone, Copy)]
struct Bounds {
    x: i64,
    y: i64,
    cx: i64,
    cy: i64,
}

#[derive(Clone)]
struct MutationOptions {
    out: Option<String>,
    backup: Option<String>,
    dry_run: bool,
    in_place: bool,
    no_validate: bool,
}

pub(crate) fn pptx_media_list(file: &str, slide_filter: i64) -> CliResult<Value> {
    ensure_pptx(file)?;
    let slides = pptx_slide_refs(file)?;
    let entries = zip_entry_names(file)?;
    let part_set = crate::zip_entry_set(&entries);
    let mut slide_values = Vec::new();
    for slide in slides {
        if slide_filter > 0 && slide.number as i64 != slide_filter {
            continue;
        }
        let slide_xml = zip_text(file, &slide.part)?;
        let rels_part = relationships_part_for(&slide.part);
        let rels = relationship_entries(file, &rels_part).unwrap_or_default();
        let clips = scan_media_pics(&slide_xml)
            .into_iter()
            .filter(|pic| pic.is_media)
            .map(|pic| media_clip_json(file, &slide.part, &slide_xml, &rels, &part_set, &pic))
            .collect::<Vec<_>>();
        slide_values.push(json!({
            "slide": slide.number,
            "partUri": format!("/{}", slide.part),
            "clips": clips,
        }));
    }
    let slides_json = if slide_filter > 0 && slide_values.is_empty() {
        Value::Null
    } else {
        Value::Array(slide_values)
    };
    Ok(json!({ "slides": slides_json }))
}

pub(crate) fn pptx_media_add(file: &str, args: &[String]) -> CliResult<Value> {
    ensure_pptx(file)?;
    let options = parse_media_mutation_options(args)?;
    let slide = crate::parse_i64_flag(args, "--slide")?.unwrap_or(0);
    if slide < 1 {
        return Err(CliError::invalid_args("--slide must be >= 1"));
    }
    let media_path = crate::parse_string_flag(args, "--file")?
        .unwrap_or_default()
        .trim()
        .to_string();
    if media_path.is_empty() {
        return Err(CliError::invalid_args(
            "--file is required (local .mp4/.m4a/.mp3/... path)",
        ));
    }
    reject_media_url(&media_path)?;
    let media_data = fs::read(&media_path)
        .map_err(|err| CliError::invalid_args(format!("failed to read --file: {err}")))?;
    let media_ext = file_extension_with_dot(&media_path);
    let kind = resolve_media_kind(
        crate::parse_string_flag(args, "--kind")?.as_deref(),
        &media_ext,
    )?;
    let media_content_type = content_type_for_media_ext(&media_ext);

    let poster_path = crate::parse_string_flag(args, "--poster")?.unwrap_or_default();
    let (poster_data, poster_content_type, poster_synthesized) = if poster_path.trim().is_empty() {
        (POSTER_PNG.to_vec(), "image/png".to_string(), true)
    } else {
        let data = fs::read(&poster_path)
            .map_err(|err| CliError::invalid_args(format!("failed to read --poster: {err}")))?;
        (data, poster_content_type_for_path(&poster_path), false)
    };

    let play_trigger = crate::parse_string_flag(args, "--play-trigger")?
        .unwrap_or_else(|| "click".to_string())
        .trim()
        .to_ascii_lowercase();
    if play_trigger != "click" && play_trigger != "none" {
        return Err(CliError::invalid_args(
            "--play-trigger must be click or none",
        ));
    }
    let emit_play_cmd = crate::has_flag(args, "--play-cmd");
    let volume = crate::parse_i64_flag(args, "--volume")?.unwrap_or(DEFAULT_MEDIA_VOLUME);
    let mute = crate::has_flag(args, "--mute");
    let insert_after = crate::parse_i64_flag(args, "--insert-after-shape")?.unwrap_or(0);
    let name = crate::parse_string_flag(args, "--name")?
        .unwrap_or_default()
        .trim()
        .to_string();

    let slides = pptx_slide_refs(file)?;
    let slide_ref = slides
        .get(slide as usize - 1)
        .ok_or_else(|| {
            CliError::target_not_found(format!(
                "slide {slide} not found (presentation has {} slides)",
                slides.len()
            ))
        })?
        .clone();
    let slide_size = pptx_slide_size(file)?;
    let bounds = resolve_media_geometry(args, slide_size)?;

    let mut warnings = Vec::new();
    if !(0..=100).contains(&volume) {
        warnings.push(format!("--volume {volume} clamped to 0..100"));
    }

    let mutation = build_add_mutation(AddMutationInput {
        file,
        slide: &slide_ref,
        media_data,
        media_ext,
        media_content_type,
        kind,
        poster_data,
        poster_content_type,
        poster_synthesized,
        name,
        bounds,
        play_trigger,
        emit_play_cmd,
        volume,
        mute,
        insert_after,
    })?;
    if mutation.poster_synthesized {
        warnings
            .push("no --poster supplied; a placeholder poster image was synthesized".to_string());
    }
    if mutation.emit_play_cmd {
        warnings.push(
            "--play-cmd emitted the Tier-D playFrom(0.0) trigger; its exact spelling is unverified against real PowerPoint".to_string(),
        );
    }

    write_media_mutation(
        file,
        &mutation.text_overrides,
        &mutation.binary_overrides,
        &options,
    )?;
    let output = media_mutation_output_path(file, &options);
    let mut result = media_add_result_json(file, output.as_deref(), options.dry_run, &mutation);
    if !warnings.is_empty() {
        result.insert("warnings".to_string(), json!(warnings));
    }
    Ok(Value::Object(result))
}

pub(crate) fn pptx_media_replace(file: &str, args: &[String]) -> CliResult<Value> {
    ensure_pptx(file)?;
    let options = parse_media_mutation_options(args)?;
    let slide = crate::parse_i64_flag(args, "--slide")?.unwrap_or(0);
    if slide < 1 {
        return Err(CliError::invalid_args("--slide must be >= 1"));
    }
    let selector = parse_media_selector(args)?;
    let media_path = crate::parse_string_flag(args, "--file")?
        .unwrap_or_default()
        .trim()
        .to_string();
    if media_path.is_empty() {
        return Err(CliError::invalid_args(
            "--file is required (local media path)",
        ));
    }
    reject_media_url(&media_path)?;
    let new_media_data = fs::read(&media_path)
        .map_err(|err| CliError::invalid_args(format!("failed to read --file: {err}")))?;
    let new_media_ext = file_extension_with_dot(&media_path);
    let new_kind = resolve_media_kind(
        crate::parse_string_flag(args, "--kind")?.as_deref(),
        &new_media_ext,
    )?;
    let new_content_type = content_type_for_media_ext(&new_media_ext);

    let poster_path = crate::parse_string_flag(args, "--poster")?.unwrap_or_default();
    let poster = if poster_path.trim().is_empty() {
        None
    } else {
        Some((
            fs::read(&poster_path)
                .map_err(|err| CliError::invalid_args(format!("failed to read --poster: {err}")))?,
            poster_content_type_for_path(&poster_path),
        ))
    };
    let expect_shape_name = crate::parse_string_flag(args, "--expect-shape-name")?
        .unwrap_or_default()
        .trim()
        .to_string();
    let expect_kind = crate::parse_string_flag(args, "--expect-media-kind")?;
    let expect_kind = if let Some(kind) = expect_kind.as_deref() {
        if kind.trim().is_empty() {
            String::new()
        } else {
            resolve_media_kind(Some(kind), "")?
        }
    } else {
        String::new()
    };
    let volume = if value_flag_present(args, "--volume") {
        Some(crate::parse_i64_flag(args, "--volume")?.unwrap_or(DEFAULT_MEDIA_VOLUME))
    } else {
        None
    };
    let mute = if value_flag_present(args, "--mute") {
        Some(crate::has_flag(args, "--mute"))
    } else {
        None
    };

    let slides = pptx_slide_refs(file)?;
    let slide_ref = slides
        .get(slide as usize - 1)
        .ok_or_else(|| {
            CliError::target_not_found(format!(
                "slide {slide} not found (presentation has {} slides)",
                slides.len()
            ))
        })?
        .clone();
    let mutation = build_replace_mutation(ReplaceMutationInput {
        file,
        slide: &slide_ref,
        selector,
        new_media_data,
        new_media_ext,
        new_kind,
        new_content_type,
        poster,
        volume,
        mute,
        expect_shape_name,
        expect_kind,
    })?;
    write_media_mutation(
        file,
        &mutation.text_overrides,
        &mutation.binary_overrides,
        &options,
    )?;
    let output = media_mutation_output_path(file, &options);
    Ok(Value::Object(media_replace_result_json(
        file,
        output.as_deref(),
        options.dry_run,
        &mutation,
    )))
}

struct AddMutationInput<'a> {
    file: &'a str,
    slide: &'a SlideRef,
    media_data: Vec<u8>,
    media_ext: String,
    media_content_type: String,
    kind: String,
    poster_data: Vec<u8>,
    poster_content_type: String,
    poster_synthesized: bool,
    name: String,
    bounds: Bounds,
    play_trigger: String,
    emit_play_cmd: bool,
    volume: i64,
    mute: bool,
    insert_after: i64,
}

struct AddMutation {
    text_overrides: BTreeMap<String, String>,
    binary_overrides: BTreeMap<String, Vec<u8>>,
    slide_number: u32,
    shape_id: u32,
    shape_name: String,
    kind: String,
    media_uri: String,
    media_content_type: String,
    poster_uri: String,
    media_rel_id: String,
    av_rel_id: String,
    poster_rel_id: String,
    play_trigger: String,
    poster_synthesized: bool,
    emit_play_cmd: bool,
}

fn build_add_mutation(input: AddMutationInput<'_>) -> CliResult<AddMutation> {
    let entries = zip_entry_names(input.file)?;
    let slide_xml = zip_text(input.file, &input.slide.part)?;
    let rels_part = relationships_part_for(&input.slide.part);
    let rels_xml = zip_text(input.file, &rels_part).unwrap_or_else(|_| {
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"></Relationships>"#.to_string()
    });
    let mut rels = crate::relationship_entries_from_xml(&rels_xml);
    let shape_id = next_shape_id(&slide_xml);
    let media_uri = allocate_numbered_part(&entries, "/ppt/media/media", &input.media_ext);
    let poster_ext = extension_for_content_type(&input.poster_content_type);
    let poster_uri = allocate_numbered_part(&entries, "/ppt/media/image", poster_ext);
    let source_uri = format!("/{}", input.slide.part);
    let media_rel_id = append_relationship(&mut rels, &source_uri, REL_TYPE_MEDIA, &media_uri);
    let av_rel_type = if input.kind == "audio" {
        REL_TYPE_AUDIO
    } else {
        REL_TYPE_VIDEO
    };
    let av_rel_id = append_relationship(&mut rels, &source_uri, av_rel_type, &media_uri);
    let poster_rel_id = append_relationship(&mut rels, &source_uri, REL_TYPE_IMAGE, &poster_uri);
    let shape_name = if input.name.is_empty() {
        media_uri
            .rsplit('/')
            .next()
            .unwrap_or("media.bin")
            .to_string()
    } else {
        input.name
    };
    let pic_xml = media_pic_xml(MediaPicSpec {
        shape_id,
        shape_name: &shape_name,
        kind: &input.kind,
        media_rel_id: &media_rel_id,
        av_rel_id: &av_rel_id,
        poster_rel_id: &poster_rel_id,
        bounds: input.bounds,
        click_play: input.play_trigger == "click",
    });
    let insert_at = media_insert_position(&slide_xml, input.insert_after as u32)?;
    let mut new_slide_xml = replace_xml_span(&slide_xml, insert_at, insert_at, &pic_xml);
    new_slide_xml = inject_media_registration(
        &new_slide_xml,
        &input.kind,
        shape_id,
        clamp_volume(input.volume),
        input.mute,
    )?;
    if input.emit_play_cmd {
        new_slide_xml = inject_play_cmd(&new_slide_xml, shape_id)?;
    }

    let mut text_overrides = BTreeMap::new();
    text_overrides.insert(input.slide.part.clone(), new_slide_xml);
    text_overrides.insert(rels_part, render_relationships_xml(&rels));
    let content_types = zip_text(input.file, "[Content_Types].xml")?;
    let content_types =
        ensure_content_type_override(content_types, &media_uri, &input.media_content_type)?;
    let content_types =
        ensure_content_type_override(content_types, &poster_uri, &input.poster_content_type)?;
    text_overrides.insert("[Content_Types].xml".to_string(), content_types);

    let mut binary_overrides = BTreeMap::new();
    binary_overrides.insert(
        media_uri.trim_start_matches('/').to_string(),
        input.media_data,
    );
    binary_overrides.insert(
        poster_uri.trim_start_matches('/').to_string(),
        input.poster_data,
    );

    Ok(AddMutation {
        text_overrides,
        binary_overrides,
        slide_number: input.slide.number,
        shape_id,
        shape_name,
        kind: input.kind,
        media_uri,
        media_content_type: input.media_content_type,
        poster_uri,
        media_rel_id,
        av_rel_id,
        poster_rel_id,
        play_trigger: input.play_trigger,
        poster_synthesized: input.poster_synthesized,
        emit_play_cmd: input.emit_play_cmd,
    })
}

enum MediaSelector {
    ShapeId(u32),
    ShapeName(String),
}

struct ReplaceMutationInput<'a> {
    file: &'a str,
    slide: &'a SlideRef,
    selector: MediaSelector,
    new_media_data: Vec<u8>,
    new_media_ext: String,
    new_kind: String,
    new_content_type: String,
    poster: Option<(Vec<u8>, String)>,
    volume: Option<i64>,
    mute: Option<bool>,
    expect_shape_name: String,
    expect_kind: String,
}

struct ReplaceMutation {
    text_overrides: BTreeMap<String, String>,
    binary_overrides: BTreeMap<String, Vec<u8>>,
    slide_number: u32,
    shape_id: u32,
    shape_name: String,
    old_kind: String,
    new_kind: String,
    old_media_uri: String,
    new_media_uri: String,
    old_content_type: String,
    new_content_type: String,
    poster_replaced: bool,
}

fn build_replace_mutation(input: ReplaceMutationInput<'_>) -> CliResult<ReplaceMutation> {
    let entries = zip_entry_names(input.file)?;
    let slide_xml = zip_text(input.file, &input.slide.part)?;
    let rels_part = relationships_part_for(&input.slide.part);
    let mut rels = relationship_entries(input.file, &rels_part)?;
    let rel_map = rels
        .iter()
        .map(|rel| (rel.id.clone(), rel.clone()))
        .collect::<BTreeMap<_, _>>();
    let pic = resolve_media_pic(&slide_xml, &input.selector)?;
    if !input.expect_shape_name.is_empty() && input.expect_shape_name != pic.shape_name {
        return Err(CliError::invalid_args(format!(
            "failed to replace media: shape name guard failed: expected {:?} but resolved {:?}",
            input.expect_shape_name, pic.shape_name
        )));
    }
    if !input.expect_kind.is_empty() && input.expect_kind != pic.kind {
        return Err(CliError::invalid_args(format!(
            "failed to replace media: media kind guard failed: expected {:?} but found {:?}",
            input.expect_kind, pic.kind
        )));
    }
    let source_uri = format!("/{}", input.slide.part);
    let old_media_uri = rel_map
        .get(&pic.av_rel_id)
        .or_else(|| rel_map.get(&pic.media_rel_id))
        .map(|rel| resolve_relationship_target(&source_uri, &rel.target))
        .unwrap_or_default();
    let old_content_type = if old_media_uri.is_empty() {
        String::new()
    } else {
        content_type_for_part(input.file, &old_media_uri)?
    };
    let mut new_media_uri = old_media_uri.clone();
    let mut binary_overrides = BTreeMap::new();
    if new_media_uri.is_empty()
        || !file_extension_with_dot(&new_media_uri).eq_ignore_ascii_case(&input.new_media_ext)
    {
        new_media_uri = allocate_numbered_part(&entries, "/ppt/media/media", &input.new_media_ext);
        retarget_relationship(&mut rels, &pic.media_rel_id, &source_uri, &new_media_uri);
        retarget_relationship(&mut rels, &pic.av_rel_id, &source_uri, &new_media_uri);
    }
    binary_overrides.insert(
        new_media_uri.trim_start_matches('/').to_string(),
        input.new_media_data,
    );

    let mut new_slide_xml = slide_xml.clone();
    if input.new_kind != pic.kind {
        new_slide_xml = flip_pic_kind(&new_slide_xml, pic.span, &input.new_kind);
        let rel_type = if input.new_kind == "audio" {
            REL_TYPE_AUDIO
        } else {
            REL_TYPE_VIDEO
        };
        set_relationship_type(&mut rels, &pic.av_rel_id, rel_type);
        new_slide_xml = flip_timing_kind(&new_slide_xml, pic.shape_id, &input.new_kind)?;
    }
    if input.volume.is_some() || input.mute.is_some() {
        new_slide_xml =
            update_media_node_attrs(&new_slide_xml, pic.shape_id, input.volume, input.mute)?;
    }

    let mut poster_replaced = false;
    if let Some((poster_data, poster_content_type)) = input.poster.as_ref() {
        let poster_ext = extension_for_content_type(poster_content_type);
        let mut poster_uri = rel_map
            .get(&pic.poster_rel_id)
            .map(|rel| resolve_relationship_target(&source_uri, &rel.target))
            .unwrap_or_default();
        if poster_uri.is_empty()
            || !file_extension_with_dot(&poster_uri).eq_ignore_ascii_case(poster_ext)
        {
            poster_uri = allocate_numbered_part(&entries, "/ppt/media/image", poster_ext);
            retarget_relationship(&mut rels, &pic.poster_rel_id, &source_uri, &poster_uri);
        }
        binary_overrides.insert(
            poster_uri.trim_start_matches('/').to_string(),
            poster_data.clone(),
        );
        poster_replaced = true;
    }

    let mut text_overrides = BTreeMap::new();
    text_overrides.insert(input.slide.part.clone(), new_slide_xml);
    text_overrides.insert(rels_part, render_relationships_xml(&rels));
    let mut content_types = zip_text(input.file, "[Content_Types].xml")?;
    content_types =
        ensure_content_type_override(content_types, &new_media_uri, &input.new_content_type)?;
    if let Some((_, poster_content_type)) = input.poster.as_ref() {
        let poster_target = rels
            .iter()
            .find(|rel| rel.id == pic.poster_rel_id)
            .map(|rel| resolve_relationship_target(&source_uri, &rel.target))
            .unwrap_or_default();
        if !poster_target.is_empty() {
            content_types =
                ensure_content_type_override(content_types, &poster_target, poster_content_type)?;
        }
    }
    text_overrides.insert("[Content_Types].xml".to_string(), content_types);

    Ok(ReplaceMutation {
        text_overrides,
        binary_overrides,
        slide_number: input.slide.number,
        shape_id: pic.shape_id,
        shape_name: pic.shape_name,
        old_kind: pic.kind,
        new_kind: input.new_kind,
        old_media_uri,
        new_media_uri,
        old_content_type,
        new_content_type: input.new_content_type,
        poster_replaced,
    })
}

fn media_clip_json(
    file: &str,
    slide_part: &str,
    slide_xml: &str,
    rels: &[RelationshipEntry],
    part_set: &BTreeSet<String>,
    pic: &PicInfo,
) -> Value {
    let source_uri = format!("/{}", slide_part.trim_start_matches('/'));
    let (media_uri, media_reason, media_external) =
        resolve_media_relationship(&source_uri, &pic.media_rel_id, rels, part_set);
    let (poster_uri, poster_reason, poster_external) =
        resolve_media_relationship(&source_uri, &pic.poster_rel_id, rels, part_set);
    let is_external = media_external || poster_external;
    let mut clip = Map::new();
    clip.insert("shapeId".to_string(), json!(pic.shape_id));
    clip.insert("shapeName".to_string(), json!(pic.shape_name));
    clip.insert("kind".to_string(), json!(pic.kind));
    clip.insert("mediaPartUri".to_string(), json!(media_uri));
    if !media_uri.is_empty() && !media_external {
        let content_type = content_type_for_part(file, &media_uri).unwrap_or_default();
        if !content_type.is_empty() {
            clip.insert("mediaContentType".to_string(), json!(content_type));
        }
    }
    if !poster_uri.is_empty() {
        clip.insert("posterPartUri".to_string(), json!(poster_uri));
    }
    clip.insert(
        "playTrigger".to_string(),
        json!(media_play_trigger(slide_xml, pic)),
    );
    let (volume, mute) = media_node_volume(slide_xml, pic.shape_id);
    clip.insert("volume".to_string(), json!(volume));
    if mute {
        clip.insert("mute".to_string(), json!(true));
    }
    if is_external {
        clip.insert("isExternal".to_string(), json!(true));
    }
    let stale_reason = if !media_reason.is_empty() {
        media_reason
    } else {
        poster_reason
    };
    if !stale_reason.is_empty() {
        clip.insert("stale".to_string(), json!(true));
        clip.insert("staleReason".to_string(), json!(stale_reason));
    }
    Value::Object(clip)
}

fn resolve_media_relationship(
    source_uri: &str,
    rel_id: &str,
    rels: &[RelationshipEntry],
    part_set: &BTreeSet<String>,
) -> (String, String, bool) {
    if rel_id.is_empty() {
        return (String::new(), String::new(), false);
    }
    let Some(rel) = rels.iter().find(|rel| rel.id == rel_id) else {
        return (String::new(), format!("dangling-rel:{rel_id}"), false);
    };
    if rel.target_mode.eq_ignore_ascii_case("External") {
        return (rel.target.clone(), String::new(), true);
    }
    let target = resolve_relationship_target(source_uri, &rel.target);
    if !part_set.contains(&target) {
        return (target.clone(), format!("missing-part:{target}"), false);
    }
    (target, String::new(), false)
}

fn media_play_trigger(slide_xml: &str, pic: &PicInfo) -> String {
    if has_play_cmd(slide_xml, pic.shape_id) {
        "cmd".to_string()
    } else if pic.has_media_hlink {
        "click".to_string()
    } else {
        "none".to_string()
    }
}

fn media_node_volume(slide_xml: &str, shape_id: u32) -> (i64, bool) {
    for node in media_node_spans(slide_xml) {
        let fragment = &slide_xml[node.start..node.end];
        if !element_targets_spid(fragment, shape_id) {
            continue;
        }
        let open_end = fragment.find('>').unwrap_or(fragment.len());
        let open_tag = &fragment[..open_end];
        let volume = parse_attr_from_tag(open_tag, "vol")
            .and_then(|value| value.parse::<i64>().ok())
            .map(|value| value / 1000)
            .unwrap_or(DEFAULT_MEDIA_VOLUME);
        let mute = parse_attr_from_tag(open_tag, "mute")
            .map(|value| value == "1" || value == "true")
            .unwrap_or(false);
        return (volume, mute);
    }
    (DEFAULT_MEDIA_VOLUME, false)
}

fn has_play_cmd(slide_xml: &str, shape_id: u32) -> bool {
    for span in element_spans(slide_xml, "cmd") {
        let fragment = &slide_xml[span.start..span.end];
        let open_end = fragment.find('>').unwrap_or(fragment.len());
        let open_tag = &fragment[..open_end];
        if parse_attr_from_tag(open_tag, "type").as_deref() != Some("call") {
            continue;
        }
        if !parse_attr_from_tag(open_tag, "cmd")
            .map(|value| value.starts_with("playFrom"))
            .unwrap_or(false)
        {
            continue;
        }
        if element_targets_spid(fragment, shape_id) {
            return true;
        }
    }
    false
}

fn element_targets_spid(fragment: &str, shape_id: u32) -> bool {
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "spTgt" =>
            {
                if attr(&e, "spid").and_then(|value| value.parse::<u32>().ok()) == Some(shape_id) {
                    return true;
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    false
}

fn scan_media_pics(xml: &str) -> Vec<PicInfo> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut path = Vec::<String>::new();
    let mut current: Option<(PicInfo, usize)> = None;
    let mut pics = Vec::new();
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if current.is_none() && name == "pic" && path.iter().any(|part| part == "spTree") {
                    current = Some((
                        PicInfo {
                            span: XmlSpan {
                                start: before,
                                end: before,
                            },
                            ..PicInfo::default()
                        },
                        path.len() + 1,
                    ));
                } else if let Some((pic, _)) = current.as_mut() {
                    note_pic_element(pic, &e, &name);
                }
                path.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if let Some((pic, _)) = current.as_mut() {
                    note_pic_element(pic, &e, &name);
                }
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if let Some((mut pic, depth)) = current.take() {
                    if path.len() == depth && name == "pic" {
                        pic.span.end = reader.buffer_position() as usize;
                        pic.is_media = pic.kind == "video"
                            || pic.kind == "audio"
                            || !pic.media_rel_id.is_empty()
                            || !pic.av_rel_id.is_empty();
                        if pic.kind.is_empty() && pic.is_media {
                            pic.kind = "unknown".to_string();
                        }
                        pics.push(pic);
                    } else {
                        current = Some((pic, depth));
                    }
                }
                path.pop();
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    pics
}

fn note_pic_element(pic: &mut PicInfo, e: &BytesStart<'_>, name: &str) {
    match name {
        "cNvPr" => {
            pic.shape_id = attr(e, "id")
                .and_then(|value| value.parse().ok())
                .unwrap_or(pic.shape_id);
            pic.shape_name = attr(e, "name").unwrap_or_else(|| pic.shape_name.clone());
        }
        "hlinkClick" => {
            if attr(e, "action").as_deref() == Some(HLINK_MEDIA_ACTION) {
                pic.has_media_hlink = true;
            }
        }
        "videoFile" => {
            pic.kind = "video".to_string();
            pic.av_rel_id = attr(e, "link").unwrap_or_else(|| pic.av_rel_id.clone());
        }
        "audioFile" => {
            pic.kind = "audio".to_string();
            pic.av_rel_id = attr(e, "link").unwrap_or_else(|| pic.av_rel_id.clone());
        }
        "media" => {
            pic.media_rel_id = attr(e, "embed").unwrap_or_else(|| pic.media_rel_id.clone());
        }
        "blip" => {
            pic.poster_rel_id = attr(e, "embed").unwrap_or_else(|| pic.poster_rel_id.clone());
        }
        _ => {}
    }
}

fn parse_media_mutation_options(args: &[String]) -> CliResult<MutationOptions> {
    let out = crate::parse_string_flag(args, "--out")?;
    let backup = crate::parse_string_flag(args, "--backup")?;
    let dry_run = crate::has_flag(args, "--dry-run");
    let in_place = crate::has_flag(args, "--in-place");
    let no_validate = crate::has_flag(args, "--no-validate");
    validate_xlsx_mutation_output_flags(out.as_deref(), in_place, backup.as_deref(), dry_run)?;
    Ok(MutationOptions {
        out,
        backup,
        dry_run,
        in_place,
        no_validate,
    })
}

fn parse_media_selector(args: &[String]) -> CliResult<MediaSelector> {
    let shape = crate::parse_i64_flag(args, "--shape")?.unwrap_or(0);
    let shape_name = crate::parse_string_flag(args, "--shape-name")?
        .unwrap_or_default()
        .trim()
        .to_string();
    if shape > 0 && !shape_name.is_empty() {
        return Err(CliError::invalid_args(
            "specify only one of --shape or --shape-name",
        ));
    }
    if shape > 0 {
        return Ok(MediaSelector::ShapeId(shape as u32));
    }
    if !shape_name.is_empty() {
        return Ok(MediaSelector::ShapeName(shape_name));
    }
    Err(CliError::invalid_args(
        "one of --shape <id> or --shape-name <name> is required",
    ))
}

fn write_media_mutation(
    file: &str,
    text_overrides: &BTreeMap<String, String>,
    binary_overrides: &BTreeMap<String, Vec<u8>>,
    options: &MutationOptions,
) -> CliResult<()> {
    let output_path = options
        .out
        .as_deref()
        .filter(|value| !value.trim().is_empty());
    let write_path = if options.dry_run || options.in_place || output_path == Some(file) {
        package_mutation_temp_path(file, "pptx-media")
    } else {
        output_path
            .ok_or_else(|| {
                CliError::invalid_args(
                    "must specify exactly one of --out, --in-place, or --dry-run",
                )
            })?
            .to_string()
    };
    copy_zip_with_binary_part_overrides_and_removals(
        file,
        &write_path,
        text_overrides,
        binary_overrides,
        &BTreeSet::new(),
    )?;
    if !options.no_validate {
        validate(&write_path, true)?;
    }
    if options.dry_run {
        let _ = fs::remove_file(&write_path);
    } else if options.in_place || output_path == Some(file) {
        if let Some(backup) = options
            .backup
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            fs::copy(file, backup)
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

fn media_mutation_output_path(file: &str, options: &MutationOptions) -> Option<String> {
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

fn resolve_media_pic(xml: &str, selector: &MediaSelector) -> CliResult<PicInfo> {
    let pics = scan_media_pics(xml);
    match selector {
        MediaSelector::ShapeId(id) => {
            for pic in pics {
                if pic.shape_id != *id {
                    continue;
                }
                if !pic.is_media {
                    return Err(CliError::invalid_args(format!(
                        "failed to replace media: shape {id} is an image, not embedded media"
                    )));
                }
                return Ok(pic);
            }
            Err(CliError::invalid_args(format!(
                "failed to replace media: media shape with id {id} not found on slide"
            )))
        }
        MediaSelector::ShapeName(name) => {
            for pic in pics {
                if pic.shape_name != *name {
                    continue;
                }
                if !pic.is_media {
                    return Err(CliError::invalid_args(format!(
                        "failed to replace media: shape {:?} is an image, not embedded media",
                        name
                    )));
                }
                return Ok(pic);
            }
            Err(CliError::invalid_args(format!(
                "failed to replace media: media shape named {:?} not found on slide",
                name
            )))
        }
    }
}

fn pptx_slide_refs(file: &str) -> CliResult<Vec<SlideRef>> {
    let presentation = zip_text(file, "ppt/presentation.xml")?;
    let slide_ids = presentation_slide_refs(&presentation);
    let rels = relationship_entries(file, "ppt/_rels/presentation.xml.rels")?;
    slide_ids
        .into_iter()
        .enumerate()
        .map(|(index, rel_id)| {
            let rel = rels
                .iter()
                .find(|rel| rel.id == rel_id)
                .ok_or_else(|| CliError::unexpected(format!("missing relationship {rel_id}")))?;
            let part = resolve_relationship_target("/ppt/presentation.xml", &rel.target)
                .trim_start_matches('/')
                .to_string();
            Ok(SlideRef {
                number: index as u32 + 1,
                part,
            })
        })
        .collect()
}

fn presentation_slide_refs(xml: &str) -> Vec<String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut refs = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "sldId" =>
            {
                if let Some(rel_id) = attr_exact(&e, "r:id") {
                    refs.push(rel_id);
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    refs
}

fn pptx_slide_size(file: &str) -> CliResult<(i64, i64)> {
    let presentation = zip_text(file, "ppt/presentation.xml")?;
    let mut reader = Reader::from_str(&presentation);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "sldSz" =>
            {
                let cx = attr(&e, "cx")
                    .and_then(|value| value.parse::<i64>().ok())
                    .unwrap_or(10 * EMU_PER_INCH);
                let cy = attr(&e, "cy")
                    .and_then(|value| value.parse::<i64>().ok())
                    .unwrap_or(EMU_PER_INCH * 15 / 2);
                return Ok((cx, cy));
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok((10 * EMU_PER_INCH, EMU_PER_INCH * 15 / 2))
}

fn resolve_media_geometry(args: &[String], slide_size: (i64, i64)) -> CliResult<Bounds> {
    let mut cx = crate::parse_i64_flag(args, "--cx")?.unwrap_or(0);
    let mut cy = crate::parse_i64_flag(args, "--cy")?.unwrap_or(0);
    if cx <= 0 {
        cx = slide_size.0 / 2;
    }
    if cy <= 0 {
        cy = slide_size.1 / 2;
    }
    let x = if value_flag_present(args, "--x") {
        crate::parse_i64_flag(args, "--x")?.unwrap_or(0)
    } else {
        ((slide_size.0 - cx) / 2).max(0)
    };
    let y = if value_flag_present(args, "--y") {
        crate::parse_i64_flag(args, "--y")?.unwrap_or(0)
    } else {
        ((slide_size.1 - cy) / 2).max(0)
    };
    Ok(Bounds { x, y, cx, cy })
}

fn next_shape_id(xml: &str) -> u32 {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut max_id = 0_u32;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "cNvPr" =>
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
    max_id.saturating_add(1).max(1)
}

fn media_insert_position(xml: &str, after_shape_id: u32) -> CliResult<usize> {
    if after_shape_id > 0
        && let Some(span) = direct_shape_spans(xml)
            .into_iter()
            .find(|shape| shape.shape_id == after_shape_id)
    {
        return Ok(span.span.end);
    }
    let sp_tree = element_content_span(xml, "spTree")?
        .ok_or_else(|| CliError::unexpected("shape tree not found in slide"))?;
    Ok(sp_tree.1)
}

#[derive(Clone)]
struct ShapeSpan {
    span: XmlSpan,
    shape_id: u32,
}

fn direct_shape_spans(xml: &str) -> Vec<ShapeSpan> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut path = Vec::<String>::new();
    let mut current: Option<(ShapeSpan, usize, String)> = None;
    let mut spans = Vec::new();
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if current.is_none()
                    && path.last().map(String::as_str) == Some("spTree")
                    && matches!(
                        name.as_str(),
                        "sp" | "pic" | "graphicFrame" | "grpSp" | "cxnSp"
                    )
                {
                    current = Some((
                        ShapeSpan {
                            span: XmlSpan {
                                start: before,
                                end: before,
                            },
                            shape_id: 0,
                        },
                        path.len() + 1,
                        name.clone(),
                    ));
                } else if let Some((shape, _, _)) = current.as_mut()
                    && name == "cNvPr"
                {
                    shape.shape_id = attr(&e, "id")
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(shape.shape_id);
                }
                path.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if let Some((shape, _, _)) = current.as_mut()
                    && name == "cNvPr"
                {
                    shape.shape_id = attr(&e, "id")
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(shape.shape_id);
                }
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if let Some((mut shape, depth, kind)) = current.take() {
                    if path.len() == depth && name == kind {
                        shape.span.end = reader.buffer_position() as usize;
                        spans.push(shape);
                    } else {
                        current = Some((shape, depth, kind));
                    }
                }
                path.pop();
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    spans
}

struct MediaPicSpec<'a> {
    shape_id: u32,
    shape_name: &'a str,
    kind: &'a str,
    media_rel_id: &'a str,
    av_rel_id: &'a str,
    poster_rel_id: &'a str,
    bounds: Bounds,
    click_play: bool,
}

fn media_pic_xml(spec: MediaPicSpec<'_>) -> String {
    let av_local = if spec.kind == "audio" {
        "audioFile"
    } else {
        "videoFile"
    };
    let hlink = if spec.click_play {
        format!(r#"<a:hlinkClick r:id="" action="{HLINK_MEDIA_ACTION}"/>"#)
    } else {
        String::new()
    };
    format!(
        r#"<p:pic><p:nvPicPr><p:cNvPr id="{shape_id}" name="{}">{hlink}</p:cNvPr><p:cNvPicPr><a:picLocks noChangeAspect="1"/></p:cNvPicPr><p:nvPr><a:{av_local} r:link="{}"/><p:extLst><p:ext uri="{MEDIA_EXT_URI}"><p14:media xmlns:p14="{P14_MEDIA_NS}" r:embed="{}"/></p:ext></p:extLst></p:nvPr></p:nvPicPr><p:blipFill><a:blip r:embed="{}"/><a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr><a:xfrm><a:off x="{}" y="{}"/><a:ext cx="{}" cy="{}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr></p:pic>"#,
        xml_attr_escape(spec.shape_name),
        xml_attr_escape(spec.av_rel_id),
        xml_attr_escape(spec.media_rel_id),
        xml_attr_escape(spec.poster_rel_id),
        spec.bounds.x,
        spec.bounds.y,
        spec.bounds.cx,
        spec.bounds.cy,
        shape_id = spec.shape_id
    )
}

fn inject_media_registration(
    xml: &str,
    kind: &str,
    shape_id: u32,
    volume: i64,
    mute: bool,
) -> CliResult<String> {
    let media_node =
        media_registration_node_xml(kind, shape_id, volume, mute, max_timing_id(xml) + 1);
    if element_spans(xml, "timing").is_empty() {
        let tm_root_id = max_timing_id(xml) + 1;
        let media_node = media_registration_node_xml(kind, shape_id, volume, mute, tm_root_id + 1);
        let timing = format!(
            r#"<p:timing><p:tnLst><p:par><p:cTn id="{tm_root_id}" dur="indefinite" restart="never" nodeType="tmRoot"><p:childTnLst>{media_node}</p:childTnLst></p:cTn></p:par></p:tnLst></p:timing>"#
        );
        return insert_slide_root_child(xml, &timing);
    }
    if let Some(child_span) = tm_root_child_tn_lst_content_span(xml)? {
        return Ok(replace_xml_span(
            xml,
            child_span.1,
            child_span.1,
            &media_node,
        ));
    }
    let timing_content = element_content_span(xml, "timing")?
        .ok_or_else(|| CliError::unexpected("timing element not found"))?;
    let tm_root_id = max_timing_id(xml) + 1;
    let media_node = media_registration_node_xml(kind, shape_id, volume, mute, tm_root_id + 1);
    let tn_lst = format!(
        r#"<p:tnLst><p:par><p:cTn id="{tm_root_id}" dur="indefinite" restart="never" nodeType="tmRoot"><p:childTnLst>{media_node}</p:childTnLst></p:cTn></p:par></p:tnLst>"#
    );
    Ok(replace_xml_span(
        xml,
        timing_content.1,
        timing_content.1,
        &tn_lst,
    ))
}

fn media_registration_node_xml(
    kind: &str,
    shape_id: u32,
    volume: i64,
    mute: bool,
    node_id: i64,
) -> String {
    let media_local = if kind == "audio" { "audio" } else { "video" };
    let mute_attr = if mute { r#" mute="1""# } else { "" };
    format!(
        r#"<p:{media_local}><p:cMediaNode vol="{}"{mute_attr}><p:cTn id="{node_id}" fill="hold" display="0"><p:stCondLst><p:cond delay="indefinite"/></p:stCondLst></p:cTn><p:tgtEl><p:spTgt spid="{shape_id}"/></p:tgtEl></p:cMediaNode></p:{media_local}>"#,
        volume * 1000
    )
}

fn inject_play_cmd(xml: &str, shape_id: u32) -> CliResult<String> {
    let click_id = max_timing_id(xml) + 1;
    let behavior_id = click_id + 1;
    let cmd = format!(
        r#"<p:par><p:cTn id="{click_id}" fill="hold" nodeType="clickEffect"><p:stCondLst><p:cond delay="indefinite"/></p:stCondLst><p:childTnLst><p:cmd type="call" cmd="{PLAY_FROM_CMD}"><p:cBhvr><p:cTn id="{behavior_id}" dur="2000" fill="hold"/><p:tgtEl><p:spTgt spid="{shape_id}"/></p:tgtEl></p:cBhvr></p:cmd></p:childTnLst></p:cTn></p:par>"#
    );
    if let Some(main_seq_child) = main_seq_child_tn_lst_content_span(xml)? {
        return Ok(replace_xml_span(
            xml,
            main_seq_child.1,
            main_seq_child.1,
            &cmd,
        ));
    }
    let Some(root_child) = tm_root_child_tn_lst_content_span(xml)? else {
        return Ok(xml.to_string());
    };
    let main_seq_id = max_timing_id(xml) + 1;
    let click_id = main_seq_id + 1;
    let behavior_id = click_id + 1;
    let cmd = format!(
        r#"<p:seq concurrent="1" nextAc="seek"><p:cTn id="{main_seq_id}" dur="indefinite" nodeType="mainSeq"><p:childTnLst><p:par><p:cTn id="{click_id}" fill="hold" nodeType="clickEffect"><p:stCondLst><p:cond delay="indefinite"/></p:stCondLst><p:childTnLst><p:cmd type="call" cmd="{PLAY_FROM_CMD}"><p:cBhvr><p:cTn id="{behavior_id}" dur="2000" fill="hold"/><p:tgtEl><p:spTgt spid="{shape_id}"/></p:tgtEl></p:cBhvr></p:cmd></p:childTnLst></p:cTn></p:par></p:childTnLst></p:cTn><p:prevCondLst><p:cond evt="onPrev" delay="0"><p:tgtEl><p:sldTgt/></p:tgtEl></p:cond></p:prevCondLst><p:nextCondLst><p:cond evt="onNext" delay="0"><p:tgtEl><p:sldTgt/></p:tgtEl></p:cond></p:nextCondLst></p:seq>"#
    );
    Ok(replace_xml_span(xml, root_child.1, root_child.1, &cmd))
}

fn insert_slide_root_child(xml: &str, child: &str) -> CliResult<String> {
    if let Some(ext_lst) = first_root_child_span(xml, "extLst")? {
        return Ok(replace_xml_span(xml, ext_lst.start, ext_lst.start, child));
    }
    let root_content = element_content_span(xml, "sld")?
        .ok_or_else(|| CliError::unexpected("slide root not found"))?;
    Ok(replace_xml_span(xml, root_content.1, root_content.1, child))
}

fn first_root_child_span(xml: &str, wanted: &str) -> CliResult<Option<XmlSpan>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut depth = 0_usize;
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                depth += 1;
                if depth == 2 && name == wanted {
                    let end = element_span_from(xml, before, wanted)?.map(|span| span.end);
                    if let Some(end) = end {
                        return Ok(Some(XmlSpan { start: before, end }));
                    }
                }
            }
            Ok(Event::Empty(e)) => {
                if depth == 1 && local_name(e.name().as_ref()) == wanted {
                    return Ok(Some(XmlSpan {
                        start: before,
                        end: reader.buffer_position() as usize,
                    }));
                }
            }
            Ok(Event::End(_)) => depth = depth.saturating_sub(1),
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(None)
}

fn tm_root_child_tn_lst_content_span(xml: &str) -> CliResult<Option<(usize, usize)>> {
    for ctn in element_spans(xml, "cTn") {
        let fragment = &xml[ctn.start..ctn.end];
        let open_end = fragment.find('>').unwrap_or(fragment.len());
        if parse_attr_from_tag(&fragment[..open_end], "nodeType").as_deref() != Some("tmRoot") {
            continue;
        }
        if let Some(child) = element_content_span(fragment, "childTnLst")? {
            return Ok(Some((ctn.start + child.0, ctn.start + child.1)));
        }
    }
    Ok(None)
}

fn main_seq_child_tn_lst_content_span(xml: &str) -> CliResult<Option<(usize, usize)>> {
    for ctn in element_spans(xml, "cTn") {
        let fragment = &xml[ctn.start..ctn.end];
        let open_end = fragment.find('>').unwrap_or(fragment.len());
        if parse_attr_from_tag(&fragment[..open_end], "nodeType").as_deref() != Some("mainSeq") {
            continue;
        }
        if let Some(child) = element_content_span(fragment, "childTnLst")? {
            return Ok(Some((ctn.start + child.0, ctn.start + child.1)));
        }
    }
    Ok(None)
}

fn media_node_spans(xml: &str) -> Vec<XmlSpan> {
    element_spans(xml, "cMediaNode")
}

fn element_spans(xml: &str, wanted: &str) -> Vec<XmlSpan> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut spans = Vec::new();
    let mut found: Option<(usize, usize)> = None;
    let mut depth = 0_usize;
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if found.is_none() && name == wanted {
                    found = Some((before, depth + 1));
                }
                depth += 1;
            }
            Ok(Event::Empty(e)) => {
                if found.is_none() && local_name(e.name().as_ref()) == wanted {
                    spans.push(XmlSpan {
                        start: before,
                        end: reader.buffer_position() as usize,
                    });
                }
            }
            Ok(Event::End(e)) => {
                if let Some((start, wanted_depth)) = found
                    && depth == wanted_depth
                    && local_name(e.name().as_ref()) == wanted
                {
                    spans.push(XmlSpan {
                        start,
                        end: reader.buffer_position() as usize,
                    });
                    found = None;
                }
                depth = depth.saturating_sub(1);
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    spans
}

fn element_span_from(xml: &str, start: usize, wanted: &str) -> CliResult<Option<XmlSpan>> {
    let fragment = &xml[start..];
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(true);
    let mut depth = 0_usize;
    loop {
        match reader.read_event() {
            Ok(Event::Start(_)) => depth += 1,
            Ok(Event::End(e)) => {
                if depth == 1 && local_name(e.name().as_ref()) == wanted {
                    return Ok(Some(XmlSpan {
                        start,
                        end: start + reader.buffer_position() as usize,
                    }));
                }
                depth = depth.saturating_sub(1);
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(None)
}

fn element_content_span(xml: &str, wanted: &str) -> CliResult<Option<(usize, usize)>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut depth = 0_usize;
    let mut found_depth = 0_usize;
    let mut open_end = 0_usize;
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                depth += 1;
                if found_depth == 0 && name == wanted {
                    found_depth = depth;
                    open_end = reader.buffer_position() as usize;
                }
            }
            Ok(Event::End(e)) => {
                if found_depth != 0
                    && depth == found_depth
                    && local_name(e.name().as_ref()) == wanted
                {
                    return Ok(Some((open_end, before)));
                }
                depth = depth.saturating_sub(1);
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(None)
}

fn max_timing_id(xml: &str) -> i64 {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut max_id = 0_i64;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == "cTn" => {
                if let Some(id) = attr(&e, "id").and_then(|value| value.parse::<i64>().ok()) {
                    max_id = max_id.max(id);
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    max_id
}

fn flip_pic_kind(xml: &str, span: XmlSpan, new_kind: &str) -> String {
    let fragment = &xml[span.start..span.end];
    let replacement = if new_kind == "audio" {
        fragment.replace("videoFile", "audioFile")
    } else {
        fragment.replace("audioFile", "videoFile")
    };
    replace_xml_span(xml, span.start, span.end, &replacement)
}

fn flip_timing_kind(xml: &str, shape_id: u32, new_kind: &str) -> CliResult<String> {
    let new_local = if new_kind == "audio" {
        "audio"
    } else {
        "video"
    };
    let old_local = if new_kind == "audio" {
        "video"
    } else {
        "audio"
    };
    for node in media_node_spans(xml) {
        let fragment = &xml[node.start..node.end];
        if !element_targets_spid(fragment, shape_id) {
            continue;
        }
        if let Some(wrapper) = parent_media_wrapper_span(xml, node, old_local) {
            let wrapper_fragment = &xml[wrapper.start..wrapper.end];
            let replacement = wrapper_fragment
                .replacen(&format!("<p:{old_local}"), &format!("<p:{new_local}"), 1)
                .replace(&format!("</p:{old_local}>"), &format!("</p:{new_local}>"));
            return Ok(replace_xml_span(
                xml,
                wrapper.start,
                wrapper.end,
                &replacement,
            ));
        }
    }
    Ok(xml.to_string())
}

fn parent_media_wrapper_span(xml: &str, node: XmlSpan, local: &str) -> Option<XmlSpan> {
    element_spans(xml, local)
        .into_iter()
        .find(|span| span.start <= node.start && span.end >= node.end)
}

fn update_media_node_attrs(
    xml: &str,
    shape_id: u32,
    volume: Option<i64>,
    mute: Option<bool>,
) -> CliResult<String> {
    for node in media_node_spans(xml) {
        let fragment = &xml[node.start..node.end];
        if !element_targets_spid(fragment, shape_id) {
            continue;
        }
        let open_end = fragment
            .find('>')
            .ok_or_else(|| CliError::unexpected("malformed cMediaNode"))?;
        let mut open_tag = fragment[..open_end].to_string();
        if let Some(volume) = volume {
            open_tag =
                set_or_insert_attr(&open_tag, "vol", &(clamp_volume(volume) * 1000).to_string());
        }
        if let Some(mute) = mute {
            if mute {
                open_tag = set_or_insert_attr(&open_tag, "mute", "1");
            } else {
                open_tag = remove_attr_from_tag(&open_tag, "mute");
            }
        }
        let replacement = format!("{}{}", open_tag, &fragment[open_end..]);
        return Ok(replace_xml_span(xml, node.start, node.end, &replacement));
    }
    Ok(xml.to_string())
}

fn parse_attr_from_tag(tag: &str, name: &str) -> Option<String> {
    let patterns = [format!(r#" {name}=""#), format!(r#":{name}=""#)];
    for pattern in patterns {
        if let Some(start) = tag.find(&pattern).map(|pos| pos + pattern.len()) {
            let end = tag[start..].find('"')? + start;
            return Some(tag[start..end].to_string());
        }
    }
    None
}

fn set_or_insert_attr(tag: &str, name: &str, value: &str) -> String {
    if parse_attr_from_tag(tag, name).is_some() {
        let pattern = format!(r#" {name}=""#);
        if let Some(start) = tag.find(&pattern).map(|pos| pos + pattern.len())
            && let Some(end) = tag[start..].find('"').map(|pos| start + pos)
        {
            return format!("{}{}{}", &tag[..start], xml_attr_escape(value), &tag[end..]);
        }
    }
    format!(r#"{tag} {name}="{}""#, xml_attr_escape(value))
}

fn remove_attr_from_tag(tag: &str, name: &str) -> String {
    let pattern = format!(r#" {name}=""#);
    if let Some(start) = tag.find(&pattern)
        && let Some(end) = tag[start + pattern.len()..]
            .find('"')
            .map(|pos| start + pattern.len() + pos + 1)
    {
        return format!("{}{}", &tag[..start], &tag[end..]);
    }
    tag.to_string()
}

fn append_relationship(
    rels: &mut Vec<RelationshipEntry>,
    source_uri: &str,
    rel_type: &str,
    target_uri: &str,
) -> String {
    let id = allocate_relationship_id(rels);
    rels.push(RelationshipEntry {
        id: id.clone(),
        rel_type: rel_type.to_string(),
        target: relationship_target_from_source_to_target(source_uri, target_uri),
        target_mode: String::new(),
    });
    id
}

fn retarget_relationship(
    rels: &mut [RelationshipEntry],
    id: &str,
    source_uri: &str,
    target_uri: &str,
) {
    for rel in rels {
        if rel.id == id {
            rel.target = relationship_target_from_source_to_target(source_uri, target_uri);
        }
    }
}

fn set_relationship_type(rels: &mut [RelationshipEntry], id: &str, rel_type: &str) {
    for rel in rels {
        if rel.id == id {
            rel.rel_type = rel_type.to_string();
        }
    }
}

fn render_relationships_xml(rels: &[RelationshipEntry]) -> String {
    let mut out = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
    );
    for rel in rels {
        out.push_str(&format!(
            r#"<Relationship Id="{}" Type="{}" Target="{}""#,
            xml_attr_escape(&rel.id),
            xml_attr_escape(&rel.rel_type),
            xml_attr_escape(&rel.target)
        ));
        if !rel.target_mode.is_empty() {
            out.push_str(&format!(
                r#" TargetMode="{}""#,
                xml_attr_escape(&rel.target_mode)
            ));
        }
        out.push_str("/>");
    }
    out.push_str("</Relationships>");
    out
}

fn allocate_numbered_part(entries: &[String], prefix: &str, ext: &str) -> String {
    let extension = if ext.starts_with('.') {
        ext.to_string()
    } else {
        format!(".{ext}")
    };
    for index in 1.. {
        let uri = format!("{prefix}{index}{extension}");
        if !zip_entry_exists(entries, &uri) {
            return uri;
        }
    }
    unreachable!("unbounded numbered part allocation")
}

fn clamp_volume(volume: i64) -> i64 {
    volume.clamp(0, 100)
}

fn ensure_pptx(file: &str) -> CliResult<()> {
    let detected = package_type(file)?;
    if detected != "pptx" {
        return Err(CliError::unsupported_type(format!(
            "unsupported type: {detected}; expected pptx"
        )));
    }
    Ok(())
}
