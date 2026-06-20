use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Map, Value, json};
use std::fs;

use crate::cli_args::value_flag_present;
use crate::pptx_readback::{
    pptx_available_shape_selectors, pptx_find_layout, pptx_layout_shape_entries,
    pptx_presentation_layouts, pptx_shape_entry_matches,
};
use crate::{
    CliError, CliResult, attr, command_arg, copy_zip_with_part_override, has_flag, local_name,
    package_mutation_temp_path, package_type, parse_i64_flag, parse_string_flag, remove_xml_span,
    replace_xml_span, validate, validate_xlsx_mutation_output_flags, xml_attr_escape,
    xml_direct_child_ranges, zip_text,
};

#[derive(Clone)]
struct PptxLayoutMutationOptions {
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

struct RenameLayoutMutation {
    layout_part: String,
    updated_xml: String,
    old_name: String,
    new_name: String,
}

struct SetBoundsMutation {
    layout_name: String,
    layout_part: String,
    updated_xml: String,
    shape_id: u32,
    shape_name: String,
    old_bounds: Bounds,
    new_bounds: Bounds,
}

struct DeleteShapeMutation {
    layout_name: String,
    layout_part: String,
    updated_xml: String,
    shape_id: u32,
    shape_name: String,
}

struct AddPlaceholderMutation {
    layout_name: String,
    layout_part: String,
    updated_xml: String,
    placeholder_type: String,
    shape_id: u32,
    shape_name: String,
    idx: i64,
}

pub(crate) fn pptx_layouts_rename(file: &str, args: &[String]) -> CliResult<Value> {
    let layout = required_string_flag(args, "--layout")?;
    let name = required_string_flag(args, "--name")?;
    let options = parse_layout_mutation_options(args)?;
    rename_pptx_layout(file, &layout, &name, options)
}

pub(crate) fn pptx_layouts_set_bounds(file: &str, args: &[String]) -> CliResult<Value> {
    let layout = required_string_flag(args, "--layout")?;
    let target = required_string_flag(args, "--target")?;
    let bounds_value = parse_string_flag(args, "--bounds")?
        .ok_or_else(|| CliError::invalid_args("--bounds must be specified in format x,y,cx,cy"))?;
    if bounds_value.trim().is_empty() {
        return Err(CliError::invalid_args(
            "--bounds must be specified in format x,y,cx,cy",
        ));
    }
    let bounds = parse_bounds(&bounds_value)
        .map_err(|message| CliError::invalid_args(format!("invalid bounds: {message}")))?;
    let options = parse_layout_mutation_options(args)?;
    set_pptx_layout_shape_bounds(file, &layout, &target, bounds, options)
}

pub(crate) fn pptx_layouts_delete_shape(file: &str, args: &[String]) -> CliResult<Value> {
    let layout = required_string_flag(args, "--layout")?;
    let target = required_string_flag(args, "--target")?;
    let options = parse_layout_mutation_options(args)?;
    delete_pptx_layout_shape(file, &layout, &target, options)
}

pub(crate) fn pptx_layouts_add_placeholder(file: &str, args: &[String]) -> CliResult<Value> {
    let layout = required_string_flag(args, "--layout")?;
    let placeholder_type = required_string_flag(args, "--type")?;
    let bounds_value = parse_string_flag(args, "--bounds")?
        .ok_or_else(|| CliError::invalid_args("--bounds must be specified in format x,y,cx,cy"))?;
    if bounds_value.trim().is_empty() {
        return Err(CliError::invalid_args(
            "--bounds must be specified in format x,y,cx,cy",
        ));
    }
    let bounds = parse_bounds(&bounds_value)
        .map_err(|message| CliError::invalid_args(format!("invalid bounds: {message}")))?;
    if bounds.cx <= 0 || bounds.cy <= 0 {
        return Err(CliError::invalid_args(
            "--bounds width and height must be positive",
        ));
    }
    let ph_type = placeholder_type.trim().to_ascii_lowercase();
    if !matches!(ph_type.as_str(), "text" | "pic") {
        return Err(CliError::invalid_args(format!(
            "invalid placeholder type {placeholder_type:?} (must be 'text' or 'pic')"
        )));
    }
    let explicit_idx = value_flag_present(args, "--idx");
    let idx = parse_i64_flag(args, "--idx")?;
    let size = parse_string_flag(args, "--size")?.unwrap_or_default();
    let orient = parse_string_flag(args, "--orient")?.unwrap_or_default();
    let options = parse_layout_mutation_options(args)?;
    add_pptx_layout_placeholder(
        file,
        &layout,
        &ph_type,
        bounds,
        idx,
        explicit_idx,
        &size,
        &orient,
        options,
    )
}

fn rename_pptx_layout(
    file: &str,
    selector: &str,
    new_name: &str,
    options: PptxLayoutMutationOptions,
) -> CliResult<Value> {
    ensure_pptx_package(file)?;
    let mutation = build_rename_layout_mutation(file, selector, new_name)?;
    let output_path = layout_mutation_output_path(file, &options);
    let staged_path =
        stage_layout_mutation(file, &mutation.layout_part, &mutation.updated_xml, &options)?;
    let result = rename_layout_result_json(file, &mutation, output_path.as_deref());
    finish_layout_mutation(file, &staged_path, &options, output_path.as_deref())?;
    Ok(result)
}

fn set_pptx_layout_shape_bounds(
    file: &str,
    selector: &str,
    target: &str,
    bounds: Bounds,
    options: PptxLayoutMutationOptions,
) -> CliResult<Value> {
    ensure_pptx_package(file)?;
    let mutation = build_set_bounds_mutation(file, selector, target, bounds)?;
    let output_path = layout_mutation_output_path(file, &options);
    let staged_path =
        stage_layout_mutation(file, &mutation.layout_part, &mutation.updated_xml, &options)?;
    let result = set_bounds_result_json(file, &mutation, output_path.as_deref());
    finish_layout_mutation(file, &staged_path, &options, output_path.as_deref())?;
    Ok(result)
}

fn delete_pptx_layout_shape(
    file: &str,
    selector: &str,
    target: &str,
    options: PptxLayoutMutationOptions,
) -> CliResult<Value> {
    ensure_pptx_package(file)?;
    let mutation = build_delete_shape_mutation(file, selector, target)?;
    let output_path = layout_mutation_output_path(file, &options);
    let staged_path =
        stage_layout_mutation(file, &mutation.layout_part, &mutation.updated_xml, &options)?;
    let result = delete_shape_result_json(file, &mutation, output_path.as_deref());
    finish_layout_mutation(file, &staged_path, &options, output_path.as_deref())?;
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
fn add_pptx_layout_placeholder(
    file: &str,
    selector: &str,
    placeholder_type: &str,
    bounds: Bounds,
    idx: Option<i64>,
    explicit_idx: bool,
    size: &str,
    orient: &str,
    options: PptxLayoutMutationOptions,
) -> CliResult<Value> {
    ensure_pptx_package(file)?;
    let mutation = build_add_placeholder_mutation(
        file,
        selector,
        placeholder_type,
        bounds,
        idx,
        explicit_idx,
        size,
        orient,
    )?;
    let output_path = layout_mutation_output_path(file, &options);
    let staged_path =
        stage_layout_mutation(file, &mutation.layout_part, &mutation.updated_xml, &options)?;
    let result = add_placeholder_result_json(file, &mutation, output_path.as_deref());
    finish_layout_mutation(file, &staged_path, &options, output_path.as_deref())?;
    Ok(result)
}

fn build_rename_layout_mutation(
    file: &str,
    selector: &str,
    new_name: &str,
) -> CliResult<RenameLayoutMutation> {
    let layouts = pptx_presentation_layouts(file)?;
    let layout = pptx_find_layout(&layouts, selector)
        .ok_or_else(|| CliError::invalid_args(format!("layout not found: {selector}")))?
        .clone();
    if layouts
        .iter()
        .any(|candidate| candidate.part_uri != layout.part_uri && candidate.name == new_name)
    {
        return Err(CliError::unexpected(format!(
            "layout name already exists: {new_name}"
        )));
    }
    let layout_xml = zip_text(file, layout.part_uri.trim_start_matches('/'))?;
    let old_name = layout.name;
    let updated_xml = set_layout_name(&layout_xml, new_name)?;
    Ok(RenameLayoutMutation {
        layout_part: layout.part_uri,
        updated_xml,
        old_name,
        new_name: new_name.to_string(),
    })
}

fn build_set_bounds_mutation(
    file: &str,
    selector: &str,
    target: &str,
    bounds: Bounds,
) -> CliResult<SetBoundsMutation> {
    let (layout, entry) = resolve_layout_shape_entry(file, selector, target)?;
    let shape_id = shape_id_from_entry(&entry)?;
    let shape_name = entry
        .get("shapeName")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let layout_xml = zip_text(file, layout.part_uri.trim_start_matches('/'))?;
    let shape = find_shape_span_by_id(&layout_xml, shape_id)?
        .ok_or_else(|| CliError::target_not_found(format!("target not found: shape:{shape_id}")))?;
    if shape.kind == "grpSp" {
        return Err(CliError::invalid_args(format!(
            "group shape bounds mutation is not supported in this slice: shape:{shape_id}"
        )));
    }
    let fragment = &layout_xml[shape.span.start..shape.span.end];
    let updated_fragment = set_shape_bounds_fragment(fragment, &shape.kind, bounds)?;
    let updated_xml = replace_xml_span(
        &layout_xml,
        shape.span.start,
        shape.span.end,
        &updated_fragment,
    );
    Ok(SetBoundsMutation {
        layout_name: layout_name_or_selector(&layout.name, selector),
        layout_part: layout.part_uri,
        updated_xml,
        shape_id,
        shape_name,
        old_bounds: bounds_from_entry(&entry),
        new_bounds: bounds,
    })
}

fn build_delete_shape_mutation(
    file: &str,
    selector: &str,
    target: &str,
) -> CliResult<DeleteShapeMutation> {
    let (layout, entry) = resolve_layout_shape_entry(file, selector, target)?;
    let shape_id = shape_id_from_entry(&entry)?;
    let shape_name = entry
        .get("shapeName")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let layout_xml = zip_text(file, layout.part_uri.trim_start_matches('/'))?;
    let shape = find_shape_span_by_id(&layout_xml, shape_id)?
        .ok_or_else(|| CliError::target_not_found(format!("target not found: shape:{shape_id}")))?;
    if shape.kind == "grpSp" {
        return Err(CliError::invalid_args(format!(
            "group shape deletion is not supported in this slice: shape:{shape_id}"
        )));
    }
    let updated_xml = remove_xml_span(&layout_xml, shape.span.start, shape.span.end);
    Ok(DeleteShapeMutation {
        layout_name: layout_name_or_selector(&layout.name, selector),
        layout_part: layout.part_uri,
        updated_xml,
        shape_id,
        shape_name,
    })
}

#[allow(clippy::too_many_arguments)]
fn build_add_placeholder_mutation(
    file: &str,
    selector: &str,
    placeholder_type: &str,
    bounds: Bounds,
    idx: Option<i64>,
    explicit_idx: bool,
    size: &str,
    orient: &str,
) -> CliResult<AddPlaceholderMutation> {
    let layouts = pptx_presentation_layouts(file)?;
    let layout = pptx_find_layout(&layouts, selector)
        .ok_or_else(|| layout_not_found_with_candidates(&layouts, selector))?
        .clone();
    let layout_xml = zip_text(file, layout.part_uri.trim_start_matches('/'))?;
    let sp_tree = find_first_element_span(&layout_xml, "spTree")?
        .ok_or_else(|| CliError::unexpected("shape tree not found in layout"))?;
    let sp_tree_fragment = &layout_xml[sp_tree.start..sp_tree.end];
    let shape_id = next_sp_tree_shape_id(sp_tree_fragment);
    let idx = if explicit_idx && idx.unwrap_or(-1) >= 0 {
        idx.unwrap_or_default()
    } else {
        allocate_next_placeholder_index(sp_tree_fragment)
    };
    let shape_name = match placeholder_type {
        "text" => format!("Content Placeholder {idx}"),
        "pic" => format!("Picture Placeholder {idx}"),
        _ => unreachable!("placeholder type validated by caller"),
    };
    let placeholder_xml = build_placeholder_xml(
        placeholder_type,
        shape_id,
        &shape_name,
        idx,
        size,
        orient,
        bounds,
    );
    let (_, content_end) = element_content_bounds(sp_tree_fragment)?;
    let insert_at = sp_tree.start + content_end;
    let updated_xml = insert_xml_at(&layout_xml, insert_at, &placeholder_xml);
    Ok(AddPlaceholderMutation {
        layout_name: layout_name_or_selector(&layout.name, selector),
        layout_part: layout.part_uri,
        updated_xml,
        placeholder_type: placeholder_type.to_string(),
        shape_id,
        shape_name,
        idx,
    })
}

fn resolve_layout_shape_entry(
    file: &str,
    selector: &str,
    target: &str,
) -> CliResult<(crate::pptx_readback::PptxLayoutInfo, Value)> {
    let (layout, shapes) = pptx_layout_shape_entries(file, selector, true)?;
    let matches = shapes
        .iter()
        .filter(|shape| pptx_shape_entry_matches(shape, target))
        .cloned()
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [entry] => Ok((layout, entry.clone())),
        [] => Err(CliError::target_not_found(format!(
            "target not found: target not found: {target} (available selectors: {})",
            pptx_available_shape_selectors(&shapes).join(", ")
        ))),
        _ => Err(CliError::target_not_found(format!(
            "target not found: ambiguous target: {target}"
        ))),
    }
}

fn rename_layout_result_json(
    file: &str,
    mutation: &RenameLayoutMutation,
    output_path: Option<&str>,
) -> Value {
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    if let Some(output_path) = output_path {
        result.insert("output".to_string(), json!(output_path));
    }
    result.insert("dryRun".to_string(), json!(output_path.is_none()));
    result.insert("layoutUri".to_string(), json!(mutation.layout_part));
    result.insert("oldName".to_string(), json!(mutation.old_name));
    result.insert("newName".to_string(), json!(mutation.new_name));
    add_pptx_layout_readback_commands(&mut result, output_path, &mutation.new_name);
    Value::Object(result)
}

fn set_bounds_result_json(
    file: &str,
    mutation: &SetBoundsMutation,
    output_path: Option<&str>,
) -> Value {
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    if let Some(output_path) = output_path {
        result.insert("output".to_string(), json!(output_path));
    }
    result.insert("dryRun".to_string(), json!(output_path.is_none()));
    result.insert("layout".to_string(), json!(mutation.layout_name));
    result.insert("layoutUri".to_string(), json!(mutation.layout_part));
    result.insert("shapeId".to_string(), json!(mutation.shape_id));
    result.insert("shapeName".to_string(), json!(mutation.shape_name));
    result.insert("oldX".to_string(), json!(mutation.old_bounds.x));
    result.insert("oldY".to_string(), json!(mutation.old_bounds.y));
    result.insert("oldCx".to_string(), json!(mutation.old_bounds.cx));
    result.insert("oldCy".to_string(), json!(mutation.old_bounds.cy));
    result.insert("newX".to_string(), json!(mutation.new_bounds.x));
    result.insert("newY".to_string(), json!(mutation.new_bounds.y));
    result.insert("newCx".to_string(), json!(mutation.new_bounds.cx));
    result.insert("newCy".to_string(), json!(mutation.new_bounds.cy));
    add_pptx_layout_readback_commands(&mut result, output_path, &mutation.layout_name);
    Value::Object(result)
}

fn delete_shape_result_json(
    file: &str,
    mutation: &DeleteShapeMutation,
    output_path: Option<&str>,
) -> Value {
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    if let Some(output_path) = output_path {
        result.insert("output".to_string(), json!(output_path));
    }
    result.insert("dryRun".to_string(), json!(output_path.is_none()));
    result.insert("layout".to_string(), json!(mutation.layout_name));
    result.insert("layoutUri".to_string(), json!(mutation.layout_part));
    result.insert("shapeId".to_string(), json!(mutation.shape_id));
    result.insert("shapeName".to_string(), json!(mutation.shape_name));
    add_pptx_layout_readback_commands(&mut result, output_path, &mutation.layout_name);
    Value::Object(result)
}

fn add_placeholder_result_json(
    file: &str,
    mutation: &AddPlaceholderMutation,
    output_path: Option<&str>,
) -> Value {
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    if let Some(output_path) = output_path {
        result.insert("output".to_string(), json!(output_path));
    }
    result.insert("dryRun".to_string(), json!(output_path.is_none()));
    result.insert("layout".to_string(), json!(mutation.layout_name));
    result.insert("layoutUri".to_string(), json!(mutation.layout_part));
    result.insert("type".to_string(), json!(mutation.placeholder_type));
    result.insert("shapeId".to_string(), json!(mutation.shape_id));
    result.insert("shapeName".to_string(), json!(mutation.shape_name));
    result.insert("idx".to_string(), json!(mutation.idx));
    add_pptx_layout_readback_commands(&mut result, output_path, &mutation.layout_name);
    Value::Object(result)
}

fn add_pptx_layout_readback_commands(
    result: &mut Map<String, Value>,
    output_path: Option<&str>,
    layout_selector: &str,
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
            "ooxml --json pptx layouts show {} --layout {}",
            command_arg(command_target),
            command_arg(layout_selector)
        )),
    );
    result.insert(
        format!("layoutsListCommand{command_suffix}"),
        json!(format!(
            "ooxml --json pptx layouts list {}",
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

fn required_string_flag(args: &[String], name: &str) -> CliResult<String> {
    let value = parse_string_flag(args, name)?
        .ok_or_else(|| CliError::invalid_args(format!("{name} must be specified")))?;
    if value.trim().is_empty() {
        return Err(CliError::invalid_args(format!("{name} must be specified")));
    }
    Ok(value)
}

fn parse_layout_mutation_options(args: &[String]) -> CliResult<PptxLayoutMutationOptions> {
    let out = parse_string_flag(args, "--out")?;
    let backup = parse_string_flag(args, "--backup")?;
    let dry_run = has_flag(args, "--dry-run");
    let in_place = has_flag(args, "--in-place");
    let no_validate = has_flag(args, "--no-validate");
    validate_xlsx_mutation_output_flags(out.as_deref(), in_place, backup.as_deref(), dry_run)?;
    Ok(PptxLayoutMutationOptions {
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

fn layout_mutation_output_path(file: &str, options: &PptxLayoutMutationOptions) -> Option<String> {
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

fn stage_layout_mutation(
    file: &str,
    layout_part: &str,
    updated_xml: &str,
    options: &PptxLayoutMutationOptions,
) -> CliResult<String> {
    let output_path = options
        .out
        .as_deref()
        .filter(|value| !value.trim().is_empty());
    let write_path = if options.dry_run || options.in_place || output_path == Some(file) {
        package_mutation_temp_path(file, "pptx-layout")
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
        layout_part.trim_start_matches('/'),
        updated_xml,
    )?;
    if !options.no_validate {
        validate(&write_path, true)?;
    }
    Ok(write_path)
}

fn finish_layout_mutation(
    file: &str,
    staged_path: &str,
    options: &PptxLayoutMutationOptions,
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

fn layout_name_or_selector(layout_name: &str, selector: &str) -> String {
    if layout_name.trim().is_empty() {
        selector.to_string()
    } else {
        layout_name.to_string()
    }
}

fn layout_not_found_with_candidates(
    layouts: &[crate::pptx_readback::PptxLayoutInfo],
    selector: &str,
) -> CliError {
    let candidates = layouts
        .iter()
        .enumerate()
        .map(|(index, layout)| {
            if layout.name.is_empty() {
                (index + 1).to_string()
            } else {
                format!("{} ({})", index + 1, layout.name)
            }
        })
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        CliError::target_not_found(format!(
            "layout not found: {selector}; discover with `ooxml --json pptx layouts list <file>`"
        ))
    } else {
        CliError::target_not_found(format!(
            "layout not found: {selector}; did you mean: {}; discover with `ooxml --json pptx layouts list <file>`",
            candidates.join(", ")
        ))
    }
}

fn shape_id_from_entry(entry: &Value) -> CliResult<u32> {
    entry
        .get("shapeId")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| CliError::unexpected("layout shape readback missing shapeId"))
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

fn set_layout_name(xml: &str, new_name: &str) -> CliResult<String> {
    let span = find_first_start_tag_span(xml, "cSld")?
        .ok_or_else(|| CliError::unexpected("layout common slide data not found"))?;
    let replacement = set_start_tag_attr(&xml[span.start..span.end], "name", new_name)?;
    Ok(replace_xml_span(xml, span.start, span.end, &replacement))
}

fn set_start_tag_attr(start_tag: &str, attr_name: &str, value: &str) -> CliResult<String> {
    let mut reader = Reader::from_str(start_tag);
    reader.config_mut().trim_text(true);
    match reader.read_event() {
        Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
            let mut out = String::new();
            out.push('<');
            out.push_str(&String::from_utf8_lossy(e.name().as_ref()));
            let mut saw_attr = false;
            for attr in e.attributes().flatten() {
                let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                if key == attr_name {
                    saw_attr = true;
                    out.push_str(&format!(r#" {key}="{}""#, xml_attr_escape(value)));
                } else {
                    out.push_str(&format!(
                        r#" {key}="{}""#,
                        String::from_utf8_lossy(attr.value.as_ref())
                    ));
                }
            }
            if !saw_attr {
                out.push_str(&format!(r#" {attr_name}="{}""#, xml_attr_escape(value)));
            }
            if start_tag.trim_end().ends_with("/>") {
                out.push_str("/>");
            } else {
                out.push('>');
            }
            Ok(out)
        }
        Ok(_) => Err(CliError::unexpected("invalid PPTX start tag")),
        Err(err) => Err(CliError::unexpected(err.to_string())),
    }
}

fn find_shape_span_by_id(xml: &str, shape_id: u32) -> CliResult<Option<ShapeSpan>> {
    let Some(sp_tree) = find_first_element_span(xml, "spTree")? else {
        return Err(CliError::unexpected("shape tree not found in layout"));
    };
    let (content_start, content_end) = element_content_bounds(&xml[sp_tree.start..sp_tree.end])?;
    let shapes = xml_direct_child_ranges(
        xml,
        sp_tree.start + content_start,
        sp_tree.start + content_end,
    )?;
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

fn build_placeholder_xml(
    placeholder_type: &str,
    shape_id: u32,
    shape_name: &str,
    idx: i64,
    size: &str,
    orient: &str,
    bounds: Bounds,
) -> String {
    let ph_type = if placeholder_type == "pic" {
        "pic"
    } else {
        "body"
    };
    let mut ph_attrs = format!(r#" type="{ph_type}" idx="{idx}""#);
    if !size.trim().is_empty() {
        ph_attrs.push_str(&format!(r#" sz="{}""#, xml_attr_escape(size.trim())));
    }
    if !orient.trim().is_empty() {
        ph_attrs.push_str(&format!(r#" orient="{}""#, xml_attr_escape(orient.trim())));
    }
    let common = format!(
        r#"<p:nvSpPr><p:cNvPr id="{shape_id}" name="{}"/><p:cNvSpPr/><p:nvPr><p:ph{ph_attrs}/></p:nvPr></p:nvSpPr><p:spPr><a:xfrm><a:off x="{}" y="{}"/><a:ext cx="{}" cy="{}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr>"#,
        xml_attr_escape(shape_name),
        bounds.x,
        bounds.y,
        bounds.cx,
        bounds.cy
    );
    if placeholder_type == "pic" {
        format!("<p:sp>{common}</p:sp>")
    } else {
        format!(
            r#"<p:sp>{common}<p:txBody><a:bodyPr rtlCol="0"/><a:lstStyle/><a:p><a:pPr lvl="0"/><a:endParaRPr lang="en-US" sz="1800" dirty="0"/></a:p></p:txBody></p:sp>"#
        )
    }
}

fn next_sp_tree_shape_id(sp_tree_fragment: &str) -> u32 {
    let mut reader = Reader::from_str(sp_tree_fragment);
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
    max_id + 1
}

fn allocate_next_placeholder_index(sp_tree_fragment: &str) -> i64 {
    let mut reader = Reader::from_str(sp_tree_fragment);
    reader.config_mut().trim_text(true);
    let mut max_idx = -1_i64;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if local_name(e.name().as_ref()) == "ph" => {
                if let Some(idx) = attr(&e, "idx").and_then(|value| value.parse::<i64>().ok()) {
                    max_idx = max_idx.max(idx);
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    max_idx + 1
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

fn find_first_start_tag_span(xml: &str, wanted_local: &str) -> CliResult<Option<XmlSpan>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == wanted_local =>
            {
                return Ok(Some(XmlSpan {
                    start: before,
                    end: reader.buffer_position() as usize,
                }));
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(None)
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
