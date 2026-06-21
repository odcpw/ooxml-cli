use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Map, Value, json};
use std::fs;

use crate::pptx_readback::{
    pptx_available_shape_selectors, pptx_shape_entry_matches, pptx_shapes_get, pptx_shapes_show,
};
use crate::{
    CliError, CliResult, attr, attr_exact, command_arg, copy_zip_with_part_override, local_name,
    package_mutation_temp_path, package_type, parse_i64_flag, parse_string_flag,
    relationship_entries_from_xml, remove_xml_span, replace_xml_span, resolve_relationship_target,
    validate, validate_xlsx_mutation_output_flags, xml_direct_child_ranges, zip_text,
};

#[derive(Clone)]
struct PptxShapeMutationOptions {
    out: Option<String>,
    backup: Option<String>,
    dry_run: bool,
    in_place: bool,
    no_validate: bool,
}

#[derive(Clone, Copy)]
struct Bounds {
    x: i64,
    y: i64,
    cx: i64,
    cy: i64,
}

#[derive(Clone, Copy)]
struct XmlSpan {
    start: usize,
    end: usize,
}

#[derive(Clone)]
struct ShapeSpan {
    span: XmlSpan,
    kind: String,
}

struct SetBoundsMutation {
    slide_part: String,
    updated_xml: String,
    shape_id: u32,
    shape_name: String,
    shape_type: String,
    target: String,
    old_bounds: Bounds,
    new_bounds: Bounds,
}

struct DeleteShapeMutation {
    slide_part: String,
    updated_xml: String,
    shape_id: u32,
    shape_name: String,
    shape_type: String,
    target: String,
}

pub(crate) fn pptx_shapes_set_bounds(file: &str, args: &[String]) -> CliResult<Value> {
    let slide = parse_i64_flag(args, "--slide")?.unwrap_or(0);
    if slide < 1 {
        return Err(CliError::invalid_args("--slide must be >= 1"));
    }
    let target = parse_string_flag(args, "--target")?
        .ok_or_else(|| CliError::invalid_args("--target is required"))?;
    if target.trim().is_empty() {
        return Err(CliError::invalid_args("--target is required"));
    }
    let bounds_value = parse_string_flag(args, "--bounds")?
        .ok_or_else(|| CliError::invalid_args("--bounds must be specified in format x,y,cx,cy"))?;
    if bounds_value.trim().is_empty() {
        return Err(CliError::invalid_args(
            "--bounds must be specified in format x,y,cx,cy",
        ));
    }
    let bounds = parse_bounds(&bounds_value)
        .map_err(|message| CliError::invalid_args(format!("invalid --bounds: {message}")))?;
    if bounds.cx <= 0 || bounds.cy <= 0 {
        return Err(CliError::invalid_args(
            "--bounds width and height must be positive",
        ));
    }
    let options = parse_shape_mutation_options(args)?;
    set_pptx_shape_bounds(file, slide as u32, &target, bounds, options)
}

pub(crate) fn pptx_shapes_delete(file: &str, args: &[String]) -> CliResult<Value> {
    let slide = parse_i64_flag(args, "--slide")?.unwrap_or(0);
    if slide < 1 {
        return Err(CliError::invalid_args("--slide must be >= 1"));
    }
    let target = parse_string_flag(args, "--target")?
        .ok_or_else(|| CliError::invalid_args("--target is required"))?;
    if target.trim().is_empty() {
        return Err(CliError::invalid_args("--target is required"));
    }
    let options = parse_shape_mutation_options(args)?;
    delete_pptx_shape(file, slide as u32, &target, options)
}

fn set_pptx_shape_bounds(
    file: &str,
    slide: u32,
    target: &str,
    bounds: Bounds,
    options: PptxShapeMutationOptions,
) -> CliResult<Value> {
    ensure_pptx_package(file)?;
    let mutation = build_set_bounds_mutation(file, slide, target, bounds)?;
    let output_path = shape_mutation_output_path(file, &options);
    let staged_path =
        stage_shape_mutation(file, &mutation.slide_part, &mutation.updated_xml, &options)?;
    let destination = read_shape_destination(
        &staged_path,
        slide,
        &mutation.target,
        output_path.as_deref(),
        &mutation.target,
        true,
        true,
    )?;
    let result = set_bounds_result_json(file, &mutation, output_path.as_deref(), destination);
    finish_shape_mutation(file, &staged_path, &options, output_path.as_deref())?;
    Ok(result)
}

fn delete_pptx_shape(
    file: &str,
    slide: u32,
    target: &str,
    options: PptxShapeMutationOptions,
) -> CliResult<Value> {
    ensure_pptx_package(file)?;
    let before = resolve_shape_entry(file, slide, target).map_err(|err| {
        if err.code == "target_not_found" {
            shape_delete_target_not_found(file, slide, target)
        } else {
            err
        }
    })?;
    let deleted = shape_destination_from_entry(&before, slide, Some(file), target);
    let mutation = build_delete_shape_mutation(file, slide, target, &before)?;
    let output_path = shape_mutation_output_path(file, &options);
    let staged_path =
        stage_shape_mutation(file, &mutation.slide_part, &mutation.updated_xml, &options)?;
    let result = delete_result_json(file, &mutation, output_path.as_deref(), deleted);
    finish_shape_mutation(file, &staged_path, &options, output_path.as_deref())?;
    Ok(result)
}

fn build_set_bounds_mutation(
    file: &str,
    slide: u32,
    target: &str,
    bounds: Bounds,
) -> CliResult<SetBoundsMutation> {
    let entry = resolve_shape_entry(file, slide, target)?;
    let shape_id = shape_id_from_entry(&entry)?;
    let shape_name = entry
        .get("shapeName")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let shape_type = entry
        .get("shapeType")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let primary = entry
        .get("primarySelector")
        .and_then(Value::as_str)
        .unwrap_or(target)
        .to_string();
    if shape_type == "grpSp" {
        return Err(CliError::invalid_args(format!(
            "group shape bounds mutation is not supported in this slice: shape:{shape_id}"
        )));
    }
    let slide_part = pptx_slide_part(file, slide)?;
    let slide_xml = zip_text(file, &slide_part)?;
    let shape = find_shape_span_by_id(&slide_xml, shape_id)?
        .ok_or_else(|| CliError::target_not_found(format!("target not found: shape:{shape_id}")))?;
    if shape.kind == "grpSp" {
        return Err(CliError::invalid_args(format!(
            "group shape bounds mutation is not supported in this slice: shape:{shape_id}"
        )));
    }
    let old_bounds = bounds_from_entry(&entry);
    let fragment = &slide_xml[shape.span.start..shape.span.end];
    let updated_fragment = set_shape_bounds_fragment(fragment, &shape.kind, bounds)?;
    let updated_xml = replace_xml_span(
        &slide_xml,
        shape.span.start,
        shape.span.end,
        &updated_fragment,
    );
    Ok(SetBoundsMutation {
        slide_part: format!("/{slide_part}"),
        updated_xml,
        shape_id,
        shape_name,
        shape_type,
        target: primary,
        old_bounds,
        new_bounds: bounds,
    })
}

fn build_delete_shape_mutation(
    file: &str,
    slide: u32,
    target: &str,
    entry: &Value,
) -> CliResult<DeleteShapeMutation> {
    let shape_id = shape_id_from_entry(entry)?;
    let shape_name = entry
        .get("shapeName")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let shape_type = entry
        .get("shapeType")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let primary = entry
        .get("primarySelector")
        .and_then(Value::as_str)
        .unwrap_or(target)
        .to_string();
    if shape_type == "grpSp" {
        return Err(CliError::invalid_args(format!(
            "group shape deletion is not supported in this slice: shape:{shape_id}"
        )));
    }
    let slide_part = pptx_slide_part(file, slide)?;
    let slide_xml = zip_text(file, &slide_part)?;
    let shape = find_shape_span_by_id_deep(&slide_xml, shape_id)?
        .ok_or_else(|| CliError::target_not_found(format!("target not found: shape:{shape_id}")))?;
    if shape.kind == "grpSp" {
        return Err(CliError::invalid_args(format!(
            "group shape deletion is not supported in this slice: shape:{shape_id}"
        )));
    }
    let updated_xml = remove_xml_span(&slide_xml, shape.span.start, shape.span.end);
    Ok(DeleteShapeMutation {
        slide_part: format!("/{slide_part}"),
        updated_xml,
        shape_id,
        shape_name,
        shape_type,
        target: primary,
    })
}

fn resolve_shape_entry(file: &str, slide: u32, target: &str) -> CliResult<Value> {
    let show = pptx_shapes_show(file, slide, true, true)?;
    let shapes = show
        .get("shapes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let matches = shapes
        .iter()
        .filter(|shape| pptx_shape_entry_matches(shape, target))
        .cloned()
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [entry] => Ok(entry.clone()),
        [] => Err(CliError::target_not_found(format!(
            "target not found: target not found: {target} (available selectors: {})",
            pptx_available_shape_selectors(&shapes).join(", ")
        ))),
        _ => Err(CliError::target_not_found(format!(
            "target not found: ambiguous target: {target}"
        ))),
    }
}

fn read_shape_destination(
    readback_path: &str,
    slide: u32,
    readback_target: &str,
    destination_file: Option<&str>,
    destination_target: &str,
    include_text: bool,
    include_bounds: bool,
) -> CliResult<Value> {
    let get = pptx_shapes_get(
        readback_path,
        slide,
        readback_target,
        include_text,
        include_bounds,
    )?;
    let entry = get
        .get("shapes")
        .and_then(Value::as_array)
        .and_then(|shapes| shapes.first())
        .ok_or_else(|| CliError::unexpected("shape readback missing destination"))?;
    Ok(shape_destination_from_entry(
        entry,
        slide,
        destination_file,
        destination_target,
    ))
}

fn shape_destination_from_entry(
    entry: &Value,
    slide: u32,
    file: Option<&str>,
    target: &str,
) -> Value {
    let mut out = Map::new();
    if let Some(file) = file {
        out.insert("file".to_string(), json!(file));
    }
    out.insert("slide".to_string(), json!(slide));
    out.insert("target".to_string(), json!(target));
    copy_json_field(entry, &mut out, "shapeId");
    copy_json_field(entry, &mut out, "shapeName");
    copy_json_field(entry, &mut out, "targetKind");
    copy_json_field(entry, &mut out, "primarySelector");
    copy_json_field(entry, &mut out, "handle");
    copy_json_field(entry, &mut out, "selectors");
    copy_json_field(entry, &mut out, "textPreview");
    copy_json_field(entry, &mut out, "bounds");
    copy_json_field(entry, &mut out, "geometry");
    copy_json_field(entry, &mut out, "imageRef");
    Value::Object(out)
}

fn copy_json_field(source: &Value, dest: &mut Map<String, Value>, key: &str) {
    if let Some(value) = source.get(key) {
        dest.insert(key.to_string(), value.clone());
    }
}

fn set_bounds_result_json(
    file: &str,
    mutation: &SetBoundsMutation,
    output_path: Option<&str>,
    destination: Value,
) -> Value {
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    if let Some(output_path) = output_path {
        result.insert("output".to_string(), json!(output_path));
    }
    result.insert("dryRun".to_string(), json!(output_path.is_none()));
    result.insert("destination".to_string(), destination.clone());
    add_pptx_shape_readback_commands(&mut result, output_path, mutation.target.as_str(), false);
    result.insert(
        "slide".to_string(),
        json!(slide_from_destination(&destination)),
    );
    result.insert("partUri".to_string(), json!(mutation.slide_part));
    result.insert("shapeId".to_string(), json!(mutation.shape_id));
    result.insert("shapeName".to_string(), json!(mutation.shape_name));
    result.insert("shapeType".to_string(), json!(mutation.shape_type));
    result.insert("target".to_string(), json!(mutation.target));
    result.insert("oldX".to_string(), json!(mutation.old_bounds.x));
    result.insert("oldY".to_string(), json!(mutation.old_bounds.y));
    result.insert("oldCx".to_string(), json!(mutation.old_bounds.cx));
    result.insert("oldCy".to_string(), json!(mutation.old_bounds.cy));
    result.insert("newX".to_string(), json!(mutation.new_bounds.x));
    result.insert("newY".to_string(), json!(mutation.new_bounds.y));
    result.insert("newCx".to_string(), json!(mutation.new_bounds.cx));
    result.insert("newCy".to_string(), json!(mutation.new_bounds.cy));
    Value::Object(result)
}

fn delete_result_json(
    file: &str,
    mutation: &DeleteShapeMutation,
    output_path: Option<&str>,
    deleted: Value,
) -> Value {
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    if let Some(output_path) = output_path {
        result.insert("output".to_string(), json!(output_path));
    }
    result.insert("dryRun".to_string(), json!(output_path.is_none()));
    result.insert("deleted".to_string(), deleted.clone());
    result.insert("slide".to_string(), json!(slide_from_destination(&deleted)));
    result.insert("partUri".to_string(), json!(mutation.slide_part));
    result.insert("shapeId".to_string(), json!(mutation.shape_id));
    result.insert("shapeName".to_string(), json!(mutation.shape_name));
    result.insert("shapeType".to_string(), json!(mutation.shape_type));
    result.insert("target".to_string(), json!(mutation.target));
    Value::Object(result)
}

fn slide_from_destination(destination: &Value) -> u32 {
    destination
        .get("slide")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or_default()
}

fn add_pptx_shape_readback_commands(
    result: &mut Map<String, Value>,
    output_path: Option<&str>,
    target: &str,
    include_text: bool,
) {
    let command_target = output_path.unwrap_or("<out.pptx>");
    let command_suffix = if output_path.is_some() {
        ""
    } else {
        "Template"
    };
    let slide = result
        .get("destination")
        .and_then(|destination| destination.get("slide"))
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let mut readback = format!(
        "ooxml --json pptx shapes get {} --slide {} --target {}",
        command_arg(command_target),
        slide,
        command_arg(target)
    );
    if include_text {
        readback.push_str(" --include-text");
    }
    readback.push_str(" --include-bounds");
    result.insert(format!("readbackCommand{command_suffix}"), json!(readback));
    result.insert(
        format!("slideReadbackCommand{command_suffix}"),
        json!(format!(
            "ooxml --json pptx slides show {} --slide {} --include-text --include-bounds",
            command_arg(command_target),
            slide
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

fn shape_id_from_entry(entry: &Value) -> CliResult<u32> {
    entry
        .get("shapeId")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| CliError::unexpected("shape readback missing shapeId"))
}

fn bounds_from_entry(entry: &Value) -> Bounds {
    let bounds = entry.get("bounds");
    Bounds {
        x: bounds
            .and_then(|value| value.get("x"))
            .and_then(Value::as_i64)
            .unwrap_or_default(),
        y: bounds
            .and_then(|value| value.get("y"))
            .and_then(Value::as_i64)
            .unwrap_or_default(),
        cx: bounds
            .and_then(|value| value.get("cx"))
            .and_then(Value::as_i64)
            .unwrap_or_default(),
        cy: bounds
            .and_then(|value| value.get("cy"))
            .and_then(Value::as_i64)
            .unwrap_or_default(),
    }
}

fn parse_shape_mutation_options(args: &[String]) -> CliResult<PptxShapeMutationOptions> {
    let out = parse_string_flag(args, "--out")?;
    let backup = parse_string_flag(args, "--backup")?;
    let dry_run = crate::has_flag(args, "--dry-run");
    let in_place = crate::has_flag(args, "--in-place");
    let no_validate = crate::has_flag(args, "--no-validate");
    validate_xlsx_mutation_output_flags(out.as_deref(), in_place, backup.as_deref(), dry_run)?;
    Ok(PptxShapeMutationOptions {
        out,
        backup,
        dry_run,
        in_place,
        no_validate,
    })
}

fn parse_bounds(value: &str) -> Result<Bounds, String> {
    let parts = value.trim().split(',').collect::<Vec<_>>();
    if parts.len() != 4 {
        return Err(format!(
            "expected 4 comma-separated values, got {}",
            parts.len()
        ));
    }
    let mut values = [0_i64; 4];
    for (index, part) in parts.iter().enumerate() {
        values[index] = part
            .trim()
            .parse::<i64>()
            .map_err(|err| format!("invalid value at position {}: {err}", index + 1))?;
    }
    Ok(Bounds {
        x: values[0],
        y: values[1],
        cx: values[2],
        cy: values[3],
    })
}

fn shape_mutation_output_path(file: &str, options: &PptxShapeMutationOptions) -> Option<String> {
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

fn stage_shape_mutation(
    file: &str,
    slide_part: &str,
    updated_xml: &str,
    options: &PptxShapeMutationOptions,
) -> CliResult<String> {
    let output_path = options
        .out
        .as_deref()
        .filter(|value| !value.trim().is_empty());
    let write_path = if options.dry_run || options.in_place || output_path == Some(file) {
        package_mutation_temp_path(file, "pptx-shape")
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

fn finish_shape_mutation(
    file: &str,
    staged_path: &str,
    options: &PptxShapeMutationOptions,
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

fn ensure_pptx_package(file: &str) -> CliResult<()> {
    let detected = package_type(file)?;
    if detected != "pptx" {
        return Err(CliError::unsupported_type(format!(
            "file is not a PPTX document (detected: {detected})"
        )));
    }
    Ok(())
}

fn pptx_slide_part(file: &str, slide: u32) -> CliResult<String> {
    let presentation = zip_text(file, "ppt/presentation.xml")?;
    let slide_rel_ids = presentation_slide_rel_ids(&presentation);
    let rel_id = slide_rel_ids.get(slide as usize - 1).ok_or_else(|| {
        CliError::invalid_args(format!(
            "slide {slide} not found (presentation has {} slides)",
            slide_rel_ids.len()
        ))
    })?;
    let rels = relationship_entries_from_xml(&zip_text(file, "ppt/_rels/presentation.xml.rels")?);
    let rel = rels
        .iter()
        .find(|candidate| candidate.id == *rel_id)
        .ok_or_else(|| CliError::unexpected(format!("missing relationship {rel_id}")))?;
    Ok(package_part_name(&resolve_relationship_target(
        "/ppt/presentation.xml",
        &rel.target,
    )))
}

fn presentation_slide_rel_ids(xml: &str) -> Vec<String> {
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

fn find_shape_span_by_id(xml: &str, shape_id: u32) -> CliResult<Option<ShapeSpan>> {
    let Some(sp_tree) = find_first_element_span(xml, "spTree")? else {
        return Err(CliError::unexpected("shape tree not found in slide"));
    };
    let (content_start, content_end) = element_content_bounds(&xml[sp_tree.start..sp_tree.end])?;
    find_shape_span_by_id_in_range(
        xml,
        sp_tree.start + content_start,
        sp_tree.start + content_end,
        shape_id,
        false,
    )
}

fn find_shape_span_by_id_deep(xml: &str, shape_id: u32) -> CliResult<Option<ShapeSpan>> {
    let Some(sp_tree) = find_first_element_span(xml, "spTree")? else {
        return Err(CliError::unexpected("shape tree not found in slide"));
    };
    let (content_start, content_end) = element_content_bounds(&xml[sp_tree.start..sp_tree.end])?;
    find_shape_span_by_id_in_range(
        xml,
        sp_tree.start + content_start,
        sp_tree.start + content_end,
        shape_id,
        true,
    )
}

fn find_shape_span_by_id_in_range(
    xml: &str,
    start: usize,
    end: usize,
    shape_id: u32,
    recurse_groups: bool,
) -> CliResult<Option<ShapeSpan>> {
    let shapes = xml_direct_child_ranges(xml, start, end)?;
    for shape in shapes
        .into_iter()
        .filter(|shape| matches!(shape.kind.as_str(), "sp" | "pic" | "graphicFrame" | "grpSp"))
    {
        let fragment = &xml[shape.start..shape.end];
        if first_c_nv_pr_id(fragment) == Some(shape_id) {
            return Ok(Some(ShapeSpan {
                span: XmlSpan {
                    start: shape.start,
                    end: shape.end,
                },
                kind: shape.kind,
            }));
        }
        if recurse_groups && shape.kind == "grpSp" {
            let (content_start, content_end) = element_content_bounds(fragment)?;
            if let Some(found) = find_shape_span_by_id_in_range(
                xml,
                shape.start + content_start,
                shape.start + content_end,
                shape_id,
                true,
            )? {
                return Ok(Some(found));
            }
        }
    }
    Ok(None)
}

fn first_c_nv_pr_id(fragment: &str) -> Option<u32> {
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "cNvPr" =>
            {
                return attr(&e, "id").and_then(|value| value.parse().ok());
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    None
}

fn set_shape_bounds_fragment(fragment: &str, kind: &str, bounds: Bounds) -> CliResult<String> {
    if kind == "graphicFrame" {
        return set_graphic_frame_bounds_fragment(fragment, bounds);
    }
    set_sp_or_pic_bounds_fragment(fragment, bounds)
}

fn set_graphic_frame_bounds_fragment(fragment: &str, bounds: Bounds) -> CliResult<String> {
    if let Some(xfrm) = find_first_element_span(fragment, "xfrm")? {
        let updated = set_xfrm_bounds(&fragment[xfrm.start..xfrm.end], bounds)?;
        return Ok(replace_xml_span(fragment, xfrm.start, xfrm.end, &updated));
    }
    let insert_at = fragment
        .find('>')
        .ok_or_else(|| CliError::unexpected("invalid PPTX shape XML"))?
        + 1;
    Ok(insert_xml_at(
        fragment,
        insert_at,
        &xfrm_fragment(bounds, "p:xfrm"),
    ))
}

fn set_sp_or_pic_bounds_fragment(fragment: &str, bounds: Bounds) -> CliResult<String> {
    let Some(sp_pr) = find_first_element_span(fragment, "spPr")? else {
        let close_start = fragment
            .rfind("</")
            .ok_or_else(|| CliError::unexpected("invalid PPTX shape XML"))?;
        return Ok(insert_xml_at(
            fragment,
            close_start,
            &format!("<p:spPr>{}</p:spPr>", xfrm_fragment(bounds, "a:xfrm")),
        ));
    };
    let sp_pr_fragment = &fragment[sp_pr.start..sp_pr.end];
    if is_self_closing_xml_element(sp_pr_fragment) {
        let replacement =
            expand_self_closing_element(sp_pr_fragment, &xfrm_fragment(bounds, "a:xfrm"))?;
        return Ok(replace_xml_span(
            fragment,
            sp_pr.start,
            sp_pr.end,
            &replacement,
        ));
    }
    if let Some(xfrm) = find_first_element_span(sp_pr_fragment, "xfrm")? {
        let updated_xfrm = set_xfrm_bounds(&sp_pr_fragment[xfrm.start..xfrm.end], bounds)?;
        let updated_sp_pr = replace_xml_span(sp_pr_fragment, xfrm.start, xfrm.end, &updated_xfrm);
        return Ok(replace_xml_span(
            fragment,
            sp_pr.start,
            sp_pr.end,
            &updated_sp_pr,
        ));
    }
    let insert_at = sp_pr_fragment
        .find('>')
        .ok_or_else(|| CliError::unexpected("invalid PPTX shape XML"))?
        + 1;
    let updated_sp_pr = insert_xml_at(sp_pr_fragment, insert_at, &xfrm_fragment(bounds, "a:xfrm"));
    Ok(replace_xml_span(
        fragment,
        sp_pr.start,
        sp_pr.end,
        &updated_sp_pr,
    ))
}

fn set_xfrm_bounds(fragment: &str, bounds: Bounds) -> CliResult<String> {
    let mut updated = if is_self_closing_xml_element(fragment) {
        expand_self_closing_element(fragment, "")?
    } else {
        fragment.to_string()
    };
    updated = replace_or_insert_xfrm_child(
        &updated,
        "off",
        &format!(r#"<a:off x="{}" y="{}"/>"#, bounds.x, bounds.y),
    )?;
    replace_or_insert_xfrm_child(
        &updated,
        "ext",
        &format!(r#"<a:ext cx="{}" cy="{}"/>"#, bounds.cx, bounds.cy),
    )
}

fn replace_or_insert_xfrm_child(
    fragment: &str,
    local_name_wanted: &str,
    replacement: &str,
) -> CliResult<String> {
    if let Some(span) = find_first_element_span(fragment, local_name_wanted)? {
        return Ok(replace_xml_span(
            fragment,
            span.start,
            span.end,
            replacement,
        ));
    }
    let close_start = fragment
        .rfind("</")
        .ok_or_else(|| CliError::unexpected("invalid PPTX transform XML"))?;
    Ok(insert_xml_at(fragment, close_start, replacement))
}

fn xfrm_fragment(bounds: Bounds, tag: &str) -> String {
    format!(
        r#"<{tag}><a:off x="{}" y="{}"/><a:ext cx="{}" cy="{}"/></{tag}>"#,
        bounds.x, bounds.y, bounds.cx, bounds.cy
    )
}

fn insert_xml_at(xml: &str, index: usize, insert: &str) -> String {
    let mut out = String::with_capacity(xml.len() + insert.len());
    out.push_str(&xml[..index]);
    out.push_str(insert);
    out.push_str(&xml[index..]);
    out
}

fn is_self_closing_xml_element(fragment: &str) -> bool {
    fragment
        .find('>')
        .is_some_and(|index| fragment[..=index].trim_end().ends_with("/>"))
}

fn expand_self_closing_element(fragment: &str, content: &str) -> CliResult<String> {
    let open_end = fragment
        .find('>')
        .ok_or_else(|| CliError::unexpected("invalid PPTX XML"))?;
    let open_tag = &fragment[..=open_end];
    let slash_index = open_tag
        .rfind('/')
        .ok_or_else(|| CliError::unexpected("invalid self-closing PPTX XML"))?;
    let start_tag = open_tag[..slash_index].trim_end();
    let tag_name = start_tag
        .trim_start()
        .strip_prefix('<')
        .and_then(|name| name.split_whitespace().next())
        .filter(|name| !name.is_empty())
        .ok_or_else(|| CliError::unexpected("invalid self-closing PPTX XML"))?;
    Ok(format!("{start_tag}>{content}</{tag_name}>"))
}

fn find_first_element_span(xml: &str, wanted_local: &str) -> CliResult<Option<XmlSpan>> {
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
                    return Ok(Some(XmlSpan {
                        start: before,
                        end: reader.buffer_position() as usize,
                    }));
                }
            }
            Ok(Event::End(e)) => {
                if let Some((start, depth)) = active.as_mut() {
                    if *depth == 1 && local_name(e.name().as_ref()) == wanted_local {
                        return Ok(Some(XmlSpan {
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

fn package_part_name(uri: &str) -> String {
    uri.trim_start_matches('/').to_string()
}

fn shape_delete_target_not_found(file: &str, slide: u32, target: &str) -> CliError {
    let candidates = pptx_shapes_show(file, slide, false, false)
        .ok()
        .and_then(|show| show.get("shapes").and_then(Value::as_array).cloned())
        .map(|shapes| {
            shapes
                .iter()
                .filter_map(|shape| {
                    shape
                        .get("primarySelector")
                        .and_then(Value::as_str)
                        .map(ToString::to_string)
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if candidates.is_empty() {
        return CliError::target_not_found(format!(
            "target not found: {target}; discover with `ooxml --json pptx shapes show <file> --slide {slide}`"
        ));
    }
    CliError::target_not_found(format!(
        "shape not found: {target}; did you mean: {}; discover with `ooxml --json pptx shapes show <file> --slide {slide}`",
        candidates.join(", ")
    ))
}
