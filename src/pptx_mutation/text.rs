use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::fs;

use crate::cli_args::{parse_bool_flag, value_flag_present};
use crate::pptx_readback::{pptx_shape_entry_matches, pptx_shapes_get, pptx_shapes_show};
use crate::{
    CliError, CliResult, RelationshipEntry, allocate_relationship_id, attr, command_arg,
    copy_zip_with_part_overrides, local_name, package_mutation_temp_path, package_type,
    parse_i64_flag, parse_string_flag, relationship_entries_from_xml, relationships_part_for,
    selector_candidates, validate, validate_xlsx_mutation_output_flags, xml_attr_escape,
    xml_direct_child_ranges, zip_text,
};

const HYPERLINK_REL_TYPE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink";
const REL_NS: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const DRAWING_REL_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

#[derive(Clone)]
struct PptxTextMutationOptions {
    out: Option<String>,
    backup: Option<String>,
    dry_run: bool,
    in_place: bool,
    no_validate: bool,
}

struct TextRunSelection {
    slide: u32,
    target: String,
    paragraph_index: usize,
    run_index: Option<usize>,
}

#[derive(Default, Clone)]
struct RunMutationOptions {
    bold: Option<bool>,
    italic: Option<bool>,
    underline: Option<String>,
    font_size: Option<f64>,
    color: Option<String>,
    font_family: Option<String>,
    hyperlink_rel_id: Option<String>,
    remove_bold: bool,
    remove_italic: bool,
    remove_underline: bool,
    remove_font_size: bool,
    remove_color: bool,
    remove_font_family: bool,
    remove_hyperlink: bool,
}

#[derive(Clone, Copy)]
struct XmlSpan {
    start: usize,
    end: usize,
}

struct TextSetMutation {
    slide_part: String,
    rels_part: String,
    updated_slide_xml: String,
    updated_rels_xml: Option<String>,
    slide: u32,
    part_uri: String,
    shape_id: u32,
    shape_name: String,
    shape_type: String,
    target: String,
    paragraph_index: usize,
    run_index: Option<usize>,
    applied_runs: Vec<usize>,
    old_properties: Vec<Value>,
    new_properties: Vec<Value>,
}

pub(crate) fn pptx_text_set(file: &str, args: &[String]) -> CliResult<Value> {
    let slide = parse_i64_flag(args, "--slide")?.unwrap_or(0);
    if slide < 1 {
        return Err(CliError::invalid_args("--slide must be >= 1"));
    }
    let target = parse_string_flag(args, "--target")?
        .ok_or_else(|| CliError::invalid_args("--target is required"))?;
    if target.trim().is_empty() {
        return Err(CliError::invalid_args("--target is required"));
    }
    let paragraph = parse_i64_flag(args, "--paragraph")?.unwrap_or(0);
    if paragraph < 0 {
        return Err(CliError::invalid_args("--paragraph must be >= 0"));
    }
    let run_index = if value_flag_present(args, "--run-index") {
        let value = parse_i64_flag(args, "--run-index")?.unwrap_or(0);
        if value < 0 {
            return Err(CliError::invalid_args("--run-index must be >= 0"));
        }
        Some(value as usize)
    } else {
        None
    };
    let (mut run_options, hyperlink) = parse_run_options(args)?;
    let options = parse_text_mutation_options(args)?;
    let selection = TextRunSelection {
        slide: slide as u32,
        target,
        paragraph_index: paragraph as usize,
        run_index,
    };
    set_pptx_text_run_properties(
        file,
        &selection,
        hyperlink.as_deref(),
        &mut run_options,
        options,
    )
}

fn parse_text_mutation_options(args: &[String]) -> CliResult<PptxTextMutationOptions> {
    let out = parse_string_flag(args, "--out")?;
    let backup = parse_string_flag(args, "--backup")?;
    let dry_run = crate::has_flag(args, "--dry-run");
    let in_place = crate::has_flag(args, "--in-place");
    let no_validate = crate::has_flag(args, "--no-validate");
    validate_xlsx_mutation_output_flags(out.as_deref(), in_place, backup.as_deref(), dry_run)?;
    Ok(PptxTextMutationOptions {
        out,
        backup,
        dry_run,
        in_place,
        no_validate,
    })
}

fn parse_run_options(args: &[String]) -> CliResult<(RunMutationOptions, Option<String>)> {
    let mut opts = RunMutationOptions::default();
    let mut any = false;

    for (set_flag, remove_flag) in [
        ("--bold", "--remove-bold"),
        ("--italic", "--remove-italic"),
        ("--underline", "--remove-underline"),
        ("--font-size", "--remove-font-size"),
        ("--color", "--remove-color"),
        ("--font-family", "--remove-font-family"),
        ("--hyperlink", "--remove-hyperlink"),
    ] {
        if flag_changed(args, set_flag) && flag_changed(args, remove_flag) {
            return Err(CliError::invalid_args(format!(
                "{set_flag} and {remove_flag} are mutually exclusive"
            )));
        }
    }

    if let Some(value) = parse_bool_flag(args, "--bold")? {
        opts.bold = Some(value);
        any = true;
    }
    if crate::has_flag(args, "--remove-bold") {
        opts.remove_bold = true;
        any = true;
    }
    if let Some(value) = parse_bool_flag(args, "--italic")? {
        opts.italic = Some(value);
        any = true;
    }
    if crate::has_flag(args, "--remove-italic") {
        opts.remove_italic = true;
        any = true;
    }
    if let Some(value) = parse_string_flag(args, "--underline")? {
        opts.underline = Some(normalize_underline_kind(&value).to_string());
        any = true;
    }
    if crate::has_flag(args, "--remove-underline") {
        opts.remove_underline = true;
        any = true;
    }
    if let Some(value) = parse_string_flag(args, "--font-size")? {
        let parsed = value
            .parse::<f64>()
            .map_err(|_| CliError::invalid_args("--font-size must be a number"))?;
        opts.font_size = Some(parsed);
        any = true;
    }
    if crate::has_flag(args, "--remove-font-size") {
        opts.remove_font_size = true;
        any = true;
    }
    if let Some(value) = parse_string_flag(args, "--color")? {
        opts.color = Some(value);
        any = true;
    }
    if crate::has_flag(args, "--remove-color") {
        opts.remove_color = true;
        any = true;
    }
    if let Some(value) = parse_string_flag(args, "--font-family")? {
        opts.font_family = Some(value);
        any = true;
    }
    if crate::has_flag(args, "--remove-font-family") {
        opts.remove_font_family = true;
        any = true;
    }

    let hyperlink = parse_string_flag(args, "--hyperlink")?;
    if hyperlink.is_some() {
        any = true;
    }
    if crate::has_flag(args, "--remove-hyperlink") {
        opts.remove_hyperlink = true;
        any = true;
    }
    if !any {
        return Err(CliError::invalid_args(
            "no styling flags provided; specify at least one of --bold/--italic/--underline/--font-size/--color/--font-family/--hyperlink (or a --remove-* flag)",
        ));
    }
    validate_run_options(&opts)?;
    Ok((opts, hyperlink))
}

fn flag_changed(args: &[String], name: &str) -> bool {
    args.iter()
        .any(|arg| arg == name || arg.starts_with(&format!("{name}=")))
}

fn normalize_underline_kind(value: &str) -> &str {
    match value {
        "single" => "sng",
        "double" => "dbl",
        other => other,
    }
}

fn validate_run_options(opts: &RunMutationOptions) -> CliResult<()> {
    if let Some(underline) = opts.underline.as_deref()
        && !valid_underline_kind(underline)
    {
        return Err(CliError::invalid_args(format!(
            "invalid underline {underline:?}"
        )));
    }
    if let Some(size) = opts.font_size
        && size <= 0.0
    {
        return Err(CliError::invalid_args(format!(
            "invalid font size {size} (must be > 0)"
        )));
    }
    if let Some(color) = opts.color.as_deref()
        && !is_rgb_hex(color)
    {
        return Err(CliError::invalid_args(format!(
            "invalid color {color:?} (expected 6 hex digits like FF0000)"
        )));
    }
    Ok(())
}

fn valid_underline_kind(value: &str) -> bool {
    matches!(
        value,
        "none"
            | "words"
            | "sng"
            | "dbl"
            | "heavy"
            | "dotted"
            | "dottedHeavy"
            | "dash"
            | "dashHeavy"
            | "dashLong"
            | "dashLongHeavy"
            | "dotDash"
            | "dotDashHeavy"
            | "dotDotDash"
            | "dotDotDashHeavy"
            | "wavy"
            | "wavyHeavy"
            | "wavyDbl"
    )
}

fn is_rgb_hex(value: &str) -> bool {
    value.len() == 6 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn set_pptx_text_run_properties(
    file: &str,
    selection: &TextRunSelection,
    hyperlink: Option<&str>,
    opts: &mut RunMutationOptions,
    options: PptxTextMutationOptions,
) -> CliResult<Value> {
    ensure_pptx_package(file)?;
    let mutation = build_text_set_mutation(
        file,
        selection.slide,
        &selection.target,
        selection.paragraph_index,
        selection.run_index,
        hyperlink,
        opts,
    )?;
    let output_path = text_mutation_output_path(file, &options);
    let staged_path = stage_text_mutation(file, &mutation, &options)?;
    let destination = read_shape_destination(
        &staged_path,
        mutation.slide,
        &mutation.target,
        output_path.as_deref(),
        true,
    )?;
    let result = text_set_result_json(file, &mutation, output_path.as_deref(), destination);
    finish_text_mutation(file, &staged_path, &options, output_path.as_deref())?;
    Ok(result)
}

fn build_text_set_mutation(
    file: &str,
    slide: u32,
    target: &str,
    paragraph_index: usize,
    run_index: Option<usize>,
    hyperlink: Option<&str>,
    opts: &mut RunMutationOptions,
) -> CliResult<TextSetMutation> {
    let show = pptx_shapes_show(file, slide, true, false)?;
    let part_uri = show
        .get("partUri")
        .and_then(Value::as_str)
        .ok_or_else(|| CliError::unexpected("PPTX shape readback missing partUri"))?
        .to_string();
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
    let entry = match matches.as_slice() {
        [entry] => entry.clone(),
        [] => {
            return Err(shape_not_found_with_candidates(slide, target, &shapes));
        }
        _ => {
            return Err(CliError::invalid_args(format!(
                "ambiguous target: {target}"
            )));
        }
    };
    if !entry
        .get("textCapable")
        .and_then(Value::as_bool)
        .unwrap_or_default()
    {
        let target_kind = entry
            .get("targetKind")
            .and_then(Value::as_str)
            .unwrap_or("shape");
        return Err(CliError::invalid_args(format!(
            "target {target} resolves to a non-text {target_kind} shape"
        )));
    }

    let shape_id = entry
        .get("shapeId")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| CliError::unexpected("shape readback missing shapeId"))?;
    let shape_name = entry
        .get("shapeName")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let shape_type = entry
        .get("shapeType")
        .and_then(Value::as_str)
        .unwrap_or("sp")
        .to_string();
    let primary = entry
        .get("primarySelector")
        .and_then(Value::as_str)
        .unwrap_or(target)
        .to_string();

    let slide_part = part_uri.trim_start_matches('/').to_string();
    let mut slide_xml = zip_text(file, &slide_part)?;
    if let Some(url) = hyperlink {
        let url = url.trim();
        if url.is_empty() {
            return Err(CliError::invalid_args("hyperlink URL cannot be empty"));
        }
        let rels_part = relationships_part_for(&slide_part);
        let rels_xml = zip_text(file, &rels_part).unwrap_or_else(|_| relationships_xml());
        let mut rels = relationship_entries_from_xml(&rels_xml);
        let rel_id = register_external_hyperlink(&mut rels, url);
        opts.hyperlink_rel_id = Some(rel_id);
        slide_xml = ensure_relationship_namespace(&slide_xml)?;
    }
    let shape = find_shape_span_by_id(&slide_xml, shape_id)?
        .ok_or_else(|| CliError::target_not_found(format!("target not found: shape:{shape_id}")))?;
    let shape_fragment = &slide_xml[shape.start..shape.end];
    let tx_body = direct_child_range(shape_fragment, "txBody")?
        .ok_or_else(|| CliError::invalid_args(format!("target {target} has no text body")))?;
    let tx_body_fragment = &shape_fragment[tx_body.start..tx_body.end];
    let (tx_content_start, tx_content_end) = element_content_bounds(tx_body_fragment)?;
    let paragraphs = xml_direct_child_ranges(tx_body_fragment, tx_content_start, tx_content_end)?
        .into_iter()
        .filter(|child| child.kind == "p")
        .collect::<Vec<_>>();
    let paragraph = paragraphs.get(paragraph_index).ok_or_else(|| {
        CliError::invalid_args(format!(
            "paragraph index {paragraph_index} out of range [0, {})",
            paragraphs.len()
        ))
    })?;
    let paragraph_fragment = &tx_body_fragment[paragraph.start..paragraph.end];
    let (p_content_start, p_content_end) = element_content_bounds(paragraph_fragment)?;
    let runs = xml_direct_child_ranges(paragraph_fragment, p_content_start, p_content_end)?
        .into_iter()
        .filter(|child| child.kind == "r")
        .collect::<Vec<_>>();
    let applied_runs = if let Some(index) = run_index {
        if index >= runs.len() {
            return Err(CliError::invalid_args(format!(
                "run index {index} not found in paragraph {paragraph_index} (paragraph has {} text run(s))",
                runs.len()
            )));
        }
        vec![index]
    } else {
        if runs.is_empty() {
            return Err(CliError::invalid_args(format!(
                "paragraph {paragraph_index} has no text runs to style"
            )));
        }
        (0..runs.len()).collect::<Vec<_>>()
    };

    let old_properties = applied_runs
        .iter()
        .map(|index| snapshot_run(&paragraph_fragment[runs[*index].start..runs[*index].end]))
        .collect::<CliResult<Vec<_>>>()?;

    let mut updated_paragraph = String::with_capacity(paragraph_fragment.len() + 128);
    let mut new_properties = Vec::with_capacity(applied_runs.len());
    let mut cursor = 0;
    for (index, run) in runs.iter().enumerate() {
        if !applied_runs.contains(&index) {
            continue;
        }
        updated_paragraph.push_str(&paragraph_fragment[cursor..run.start]);
        let run_fragment = &paragraph_fragment[run.start..run.end];
        let updated_run = apply_run_options(run_fragment, opts)?;
        new_properties.push(snapshot_run(&updated_run)?);
        updated_paragraph.push_str(&updated_run);
        cursor = run.end;
    }
    updated_paragraph.push_str(&paragraph_fragment[cursor..]);

    let updated_tx_body = replace_xml_span(
        tx_body_fragment,
        paragraph.start,
        paragraph.end,
        &updated_paragraph,
    );
    let updated_shape =
        replace_xml_span(shape_fragment, tx_body.start, tx_body.end, &updated_tx_body);
    let updated_slide_xml = replace_xml_span(&slide_xml, shape.start, shape.end, &updated_shape);

    let rels_part = relationships_part_for(&slide_part);
    let updated_rels_xml = if opts.hyperlink_rel_id.is_some() {
        let rels_xml = zip_text(file, &rels_part).unwrap_or_else(|_| relationships_xml());
        let mut rels = relationship_entries_from_xml(&rels_xml);
        if let Some(url) = hyperlink {
            register_external_hyperlink(&mut rels, url.trim());
        }
        Some(render_relationships_xml(&rels))
    } else {
        None
    };

    Ok(TextSetMutation {
        slide_part,
        rels_part,
        updated_slide_xml,
        updated_rels_xml,
        slide,
        part_uri,
        shape_id,
        shape_name,
        shape_type,
        target: primary,
        paragraph_index,
        run_index,
        applied_runs,
        old_properties,
        new_properties,
    })
}

fn shape_not_found_with_candidates(slide: u32, target: &str, shapes: &[Value]) -> CliError {
    let primary_selectors = shapes
        .iter()
        .filter_map(|shape| {
            let primary = shape.get("primarySelector")?.as_str()?.to_string();
            let selectors = shape
                .get("selectors")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            Some((primary, selectors))
        })
        .collect::<Vec<_>>();
    let borrowed = primary_selectors
        .iter()
        .map(|(primary, selectors)| (primary.as_str(), selectors.as_slice()))
        .collect::<Vec<_>>();
    let candidates = selector_candidates(&borrowed, target, 3);
    let discovery = format!("ooxml --json pptx shapes show <file> --slide {slide}");
    if candidates.is_empty() {
        return CliError::target_not_found(format!(
            "target not found: target not found: {target}; discover with `{discovery}`"
        ));
    }
    CliError::target_not_found(format!(
        "shape not found: {target}; did you mean: {}; discover with `{discovery}`",
        candidates.join(", ")
    ))
}

fn read_shape_destination(
    readback_path: &str,
    slide: u32,
    target: &str,
    destination_file: Option<&str>,
    include_text: bool,
) -> CliResult<Value> {
    let get = pptx_shapes_get(readback_path, slide, target, include_text, false)?;
    let entry = get
        .get("shapes")
        .and_then(Value::as_array)
        .and_then(|shapes| shapes.first())
        .ok_or_else(|| CliError::unexpected("shape readback missing destination"))?;
    let mut out = Map::new();
    if let Some(file) = destination_file {
        out.insert("file".to_string(), json!(file));
    }
    out.insert("slide".to_string(), json!(slide));
    out.insert("target".to_string(), json!(target));
    for key in [
        "shapeId",
        "shapeName",
        "targetKind",
        "primarySelector",
        "handle",
        "selectors",
        "textPreview",
    ] {
        if let Some(value) = entry.get(key) {
            out.insert(key.to_string(), value.clone());
        }
    }
    Ok(Value::Object(out))
}

fn text_set_result_json(
    file: &str,
    mutation: &TextSetMutation,
    output_path: Option<&str>,
    destination: Value,
) -> Value {
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    if let Some(output_path) = output_path {
        result.insert("output".to_string(), json!(output_path));
    }
    result.insert("dryRun".to_string(), json!(output_path.is_none()));
    result.insert("destination".to_string(), destination);
    add_pptx_text_readback_commands(&mut result, output_path, mutation.slide, &mutation.target);
    result.insert("slide".to_string(), json!(mutation.slide));
    result.insert("partUri".to_string(), json!(mutation.part_uri));
    result.insert("shapeId".to_string(), json!(mutation.shape_id));
    result.insert("shapeName".to_string(), json!(mutation.shape_name));
    result.insert("shapeType".to_string(), json!(mutation.shape_type));
    result.insert("target".to_string(), json!(mutation.target));
    result.insert(
        "paragraphIndex".to_string(),
        json!(mutation.paragraph_index),
    );
    if let Some(run_index) = mutation.run_index {
        result.insert("runIndex".to_string(), json!(run_index));
    }
    result.insert("appliedRuns".to_string(), json!(mutation.applied_runs));
    result.insert(
        "oldProperties".to_string(),
        Value::Array(mutation.old_properties.clone()),
    );
    result.insert(
        "newProperties".to_string(),
        Value::Array(mutation.new_properties.clone()),
    );
    Value::Object(result)
}

fn add_pptx_text_readback_commands(
    result: &mut Map<String, Value>,
    output_path: Option<&str>,
    slide: u32,
    target: &str,
) {
    let command_target = output_path.unwrap_or("<out.pptx>");
    let suffix = if output_path.is_some() {
        ""
    } else {
        "Template"
    };
    result.insert(
        format!("readbackCommand{suffix}"),
        json!(format!(
            "ooxml --json pptx shapes get {} --slide {} --target {} --include-text",
            command_arg(command_target),
            slide,
            command_arg(target)
        )),
    );
    result.insert(
        format!("slideReadbackCommand{suffix}"),
        json!(format!(
            "ooxml --json pptx slides show {} --slide {} --include-text --include-bounds",
            command_arg(command_target),
            slide
        )),
    );
    result.insert(
        format!("validateCommand{suffix}"),
        json!(format!(
            "ooxml validate --strict {}",
            command_arg(command_target)
        )),
    );
    result.insert(
        format!("renderCommand{suffix}"),
        json!(format!(
            "ooxml pptx render {} --out render-check",
            command_arg(command_target)
        )),
    );
}

fn text_mutation_output_path(file: &str, options: &PptxTextMutationOptions) -> Option<String> {
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

fn stage_text_mutation(
    file: &str,
    mutation: &TextSetMutation,
    options: &PptxTextMutationOptions,
) -> CliResult<String> {
    let output_path = options
        .out
        .as_deref()
        .filter(|value| !value.trim().is_empty());
    let write_path = if options.dry_run || options.in_place || output_path == Some(file) {
        package_mutation_temp_path(file, "pptx-text")
    } else {
        output_path
            .ok_or_else(|| {
                CliError::invalid_args(
                    "must specify exactly one of --out, --in-place, or --dry-run",
                )
            })?
            .to_string()
    };
    let mut overrides = BTreeMap::new();
    overrides.insert(
        mutation.slide_part.clone(),
        mutation.updated_slide_xml.clone(),
    );
    if let Some(rels_xml) = mutation.updated_rels_xml.as_ref() {
        overrides.insert(mutation.rels_part.clone(), rels_xml.clone());
    }
    copy_zip_with_part_overrides(file, &write_path, &overrides)?;
    if !options.no_validate {
        validate(&write_path, true)?;
    }
    Ok(write_path)
}

fn finish_text_mutation(
    file: &str,
    staged_path: &str,
    options: &PptxTextMutationOptions,
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

fn snapshot_run(run_fragment: &str) -> CliResult<Value> {
    let mut snap = Map::new();
    if let Some(text) = direct_child_text(run_fragment, "t")? {
        snap.insert("text".to_string(), json!(text));
    }
    let Some(r_pr) = direct_child_range(run_fragment, "rPr")? else {
        return Ok(Value::Object(snap));
    };
    let r_pr_fragment = &run_fragment[r_pr.start..r_pr.end];
    let attrs = first_element_attrs(r_pr_fragment)?;
    if let Some(value) = attrs.get("b") {
        snap.insert(
            "bold".to_string(),
            json!(value == "1" || value.eq_ignore_ascii_case("true")),
        );
    }
    if let Some(value) = attrs.get("i") {
        snap.insert(
            "italic".to_string(),
            json!(value == "1" || value.eq_ignore_ascii_case("true")),
        );
    }
    if let Some(value) = attrs.get("u") {
        snap.insert("underline".to_string(), json!(value));
    }
    if let Some(value) = attrs.get("sz")
        && let Ok(size) = value.parse::<f64>()
    {
        snap.insert("fontSize".to_string(), json_number(size / 100.0));
    }
    if let Some(latin) = direct_child_range(r_pr_fragment, "latin")? {
        let attrs = first_element_attrs(&r_pr_fragment[latin.start..latin.end])?;
        if let Some(typeface) = attrs.get("typeface") {
            snap.insert("fontFamily".to_string(), json!(typeface));
        }
    }
    if let Some(solid_fill) = direct_child_range(r_pr_fragment, "solidFill")?
        && let Some(srgb) =
            find_first_element_span(&r_pr_fragment[solid_fill.start..solid_fill.end], "srgbClr")?
    {
        let fill_fragment = &r_pr_fragment[solid_fill.start..solid_fill.end];
        let attrs = first_element_attrs(&fill_fragment[srgb.start..srgb.end])?;
        if let Some(color) = attrs.get("val") {
            snap.insert("color".to_string(), json!(color));
        }
    }
    if let Some(hlink) = direct_child_range(r_pr_fragment, "hlinkClick")? {
        let attrs = first_element_attrs(&r_pr_fragment[hlink.start..hlink.end])?;
        if let Some(id) = attrs.get("id") {
            snap.insert("hyperlink".to_string(), json!(id));
        }
    }
    Ok(Value::Object(snap))
}

fn direct_child_text(fragment: &str, wanted: &str) -> CliResult<Option<String>> {
    let Some(child) = direct_child_range(fragment, wanted)? else {
        return Ok(None);
    };
    let child_fragment = &fragment[child.start..child.end];
    let mut reader = Reader::from_str(child_fragment);
    reader.config_mut().trim_text(false);
    let mut text = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Text(e)) => text.push_str(&String::from_utf8_lossy(e.as_ref())),
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(Some(text))
}

fn json_number(value: f64) -> Value {
    if value.fract() == 0.0 {
        json!(value as i64)
    } else {
        json!(value)
    }
}

fn apply_run_options(run_fragment: &str, opts: &RunMutationOptions) -> CliResult<String> {
    let mut r_pr_span = direct_child_range(run_fragment, "rPr")?;
    let mut working_run = run_fragment.to_string();
    if r_pr_span.is_none() {
        let insert_at = first_child_insert_position(run_fragment)?;
        working_run = insert_xml_at(run_fragment, insert_at, "<a:rPr/>");
        r_pr_span = direct_child_range(&working_run, "rPr")?;
    }
    let r_pr_span = r_pr_span.ok_or_else(|| CliError::unexpected("failed to create a:rPr"))?;
    let r_pr = &working_run[r_pr_span.start..r_pr_span.end];
    let mut updated_r_pr = r_pr.to_string();
    if opts.remove_bold {
        updated_r_pr = remove_start_tag_attr(&updated_r_pr, "b")?;
    } else if let Some(value) = opts.bold {
        updated_r_pr = set_start_tag_attr(&updated_r_pr, "b", bool_attr(value))?;
    }
    if opts.remove_italic {
        updated_r_pr = remove_start_tag_attr(&updated_r_pr, "i")?;
    } else if let Some(value) = opts.italic {
        updated_r_pr = set_start_tag_attr(&updated_r_pr, "i", bool_attr(value))?;
    }
    if opts.remove_underline {
        updated_r_pr = remove_start_tag_attr(&updated_r_pr, "u")?;
    } else if let Some(value) = opts.underline.as_deref() {
        updated_r_pr = set_start_tag_attr(&updated_r_pr, "u", value)?;
    }
    if opts.remove_font_size {
        updated_r_pr = remove_start_tag_attr(&updated_r_pr, "sz")?;
    } else if let Some(value) = opts.font_size {
        updated_r_pr = set_start_tag_attr(&updated_r_pr, "sz", &format!("{:.0}", value * 100.0))?;
    }
    if opts.remove_color {
        updated_r_pr = remove_rpr_child(&updated_r_pr, "solidFill")?;
    } else if let Some(color) = opts.color.as_deref() {
        updated_r_pr = remove_rpr_child(&updated_r_pr, "solidFill")?;
        updated_r_pr = insert_rpr_child_ordered(
            &updated_r_pr,
            "solidFill",
            &format!(
                r#"<a:solidFill><a:srgbClr val="{}"/></a:solidFill>"#,
                xml_attr_escape(&color.to_ascii_uppercase())
            ),
        )?;
    }
    if opts.remove_font_family {
        updated_r_pr = remove_rpr_child(&updated_r_pr, "latin")?;
    } else if let Some(font_family) = opts.font_family.as_deref() {
        updated_r_pr = remove_rpr_child(&updated_r_pr, "latin")?;
        updated_r_pr = insert_rpr_child_ordered(
            &updated_r_pr,
            "latin",
            &format!(r#"<a:latin typeface="{}"/>"#, xml_attr_escape(font_family)),
        )?;
    }
    if opts.remove_hyperlink {
        updated_r_pr = remove_rpr_child(&updated_r_pr, "hlinkClick")?;
    } else if let Some(rel_id) = opts.hyperlink_rel_id.as_deref() {
        updated_r_pr = remove_rpr_child(&updated_r_pr, "hlinkClick")?;
        updated_r_pr = insert_rpr_child_ordered(
            &updated_r_pr,
            "hlinkClick",
            &format!(r#"<a:hlinkClick r:id="{}"/>"#, xml_attr_escape(rel_id)),
        )?;
    }
    Ok(replace_xml_span(
        &working_run,
        r_pr_span.start,
        r_pr_span.end,
        &updated_r_pr,
    ))
}

fn first_child_insert_position(fragment: &str) -> CliResult<usize> {
    fragment
        .find('>')
        .map(|index| index + 1)
        .ok_or_else(|| CliError::unexpected("invalid PPTX run XML"))
}

fn bool_attr(value: bool) -> &'static str {
    if value { "1" } else { "0" }
}

fn set_start_tag_attr(fragment: &str, name: &str, value: &str) -> CliResult<String> {
    let open_end = fragment
        .find('>')
        .ok_or_else(|| CliError::unexpected("invalid PPTX XML"))?;
    let open_tag = &fragment[..=open_end];
    let close = if open_tag.trim_end().ends_with("/>") {
        "/>"
    } else {
        ">"
    };
    let mut tag = open_tag
        .trim_end_matches('>')
        .trim_end_matches('/')
        .trim_end()
        .to_string();
    tag = remove_attr_from_start_tag(&tag, name);
    tag.push(' ');
    tag.push_str(name);
    tag.push_str("=\"");
    tag.push_str(&xml_attr_escape(value));
    tag.push('"');
    tag.push_str(close);
    Ok(format!("{tag}{}", &fragment[open_end + 1..]))
}

fn remove_start_tag_attr(fragment: &str, name: &str) -> CliResult<String> {
    let open_end = fragment
        .find('>')
        .ok_or_else(|| CliError::unexpected("invalid PPTX XML"))?;
    let open_tag = remove_attr_from_start_tag(&fragment[..=open_end], name);
    Ok(format!("{open_tag}{}", &fragment[open_end + 1..]))
}

fn remove_attr_from_start_tag(tag: &str, name: &str) -> String {
    let mut out = String::new();
    let bytes = tag.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index].is_ascii_whitespace() {
            let attr_start = index;
            index += 1;
            while index < bytes.len() && bytes[index].is_ascii_whitespace() {
                index += 1;
            }
            let name_start = index;
            while index < bytes.len()
                && !bytes[index].is_ascii_whitespace()
                && bytes[index] != b'='
                && bytes[index] != b'>'
                && bytes[index] != b'/'
            {
                index += 1;
            }
            let attr_name = &tag[name_start..index];
            while index < bytes.len() && bytes[index].is_ascii_whitespace() {
                index += 1;
            }
            if index < bytes.len() && bytes[index] == b'=' {
                index += 1;
                while index < bytes.len() && bytes[index].is_ascii_whitespace() {
                    index += 1;
                }
                if index < bytes.len() && (bytes[index] == b'"' || bytes[index] == b'\'') {
                    let quote = bytes[index];
                    index += 1;
                    while index < bytes.len() && bytes[index] != quote {
                        index += 1;
                    }
                    if index < bytes.len() {
                        index += 1;
                    }
                }
            }
            if local_attr_name(attr_name) == name {
                continue;
            }
            out.push_str(&tag[attr_start..index]);
            continue;
        }
        out.push(bytes[index] as char);
        index += 1;
    }
    out
}

fn local_attr_name(name: &str) -> &str {
    name.rsplit_once(':')
        .map(|(_, local)| local)
        .unwrap_or(name)
}

fn remove_rpr_child(fragment: &str, local: &str) -> CliResult<String> {
    let working = expand_self_closing(fragment)?;
    let (content_start, content_end) = element_content_bounds(&working)?;
    let children = xml_direct_child_ranges(&working, content_start, content_end)?;
    let mut out = String::with_capacity(working.len());
    let mut cursor = 0;
    for child in children.into_iter().filter(|child| child.kind == local) {
        out.push_str(&working[cursor..child.start]);
        cursor = child.end;
    }
    out.push_str(&working[cursor..]);
    Ok(out)
}

fn insert_rpr_child_ordered(fragment: &str, local: &str, child_xml: &str) -> CliResult<String> {
    let working = expand_self_closing(fragment)?;
    let (content_start, content_end) = element_content_bounds(&working)?;
    let children = xml_direct_child_ranges(&working, content_start, content_end)?;
    let rank = rpr_child_rank(local);
    let insert_at = children
        .iter()
        .find(|child| rpr_child_rank(&child.kind) > rank)
        .map(|child| child.start)
        .unwrap_or(content_end);
    Ok(insert_xml_at(&working, insert_at, child_xml))
}

fn rpr_child_rank(local: &str) -> usize {
    [
        "ln",
        "noFill",
        "solidFill",
        "gradFill",
        "blipFill",
        "pattFill",
        "grpFill",
        "effectLst",
        "effectDag",
        "highlight",
        "uLnTx",
        "uLn",
        "uFillTx",
        "uFill",
        "latin",
        "ea",
        "cs",
        "sym",
        "hlinkClick",
        "hlinkMouseOver",
        "rtl",
        "extLst",
    ]
    .iter()
    .position(|candidate| *candidate == local)
    .unwrap_or(usize::MAX)
}

fn register_external_hyperlink(rels: &mut Vec<RelationshipEntry>, url: &str) -> String {
    if let Some(existing) = rels.iter().find(|rel| {
        rel.rel_type == HYPERLINK_REL_TYPE && rel.target_mode == "External" && rel.target == url
    }) {
        return existing.id.clone();
    }
    let id = allocate_relationship_id(rels);
    rels.push(RelationshipEntry {
        id: id.clone(),
        rel_type: HYPERLINK_REL_TYPE.to_string(),
        target: url.to_string(),
        target_mode: "External".to_string(),
    });
    id
}

fn render_relationships_xml(rels: &[RelationshipEntry]) -> String {
    let mut xml =
        format!(r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="{REL_NS}">"#);
    for rel in rels {
        xml.push_str(&format!(
            r#"<Relationship Id="{}" Type="{}" Target="{}""#,
            xml_attr_escape(&rel.id),
            xml_attr_escape(&rel.rel_type),
            xml_attr_escape(&rel.target)
        ));
        if !rel.target_mode.is_empty() {
            xml.push_str(&format!(
                r#" TargetMode="{}""#,
                xml_attr_escape(&rel.target_mode)
            ));
        }
        xml.push_str("/>");
    }
    xml.push_str("</Relationships>");
    xml
}

fn relationships_xml() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="{REL_NS}"></Relationships>"#
    )
}

fn ensure_relationship_namespace(xml: &str) -> CliResult<String> {
    if xml.contains("xmlns:r=") {
        return Ok(xml.to_string());
    }
    let open_end = xml
        .find('>')
        .ok_or_else(|| CliError::unexpected("invalid PPTX slide XML"))?;
    let insert = format!(r#" xmlns:r="{DRAWING_REL_NS}""#);
    Ok(insert_xml_at(xml, open_end, &insert))
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

fn find_shape_span_by_id(xml: &str, shape_id: u32) -> CliResult<Option<XmlSpan>> {
    let Some(sp_tree) = find_first_element_span(xml, "spTree")? else {
        return Err(CliError::unexpected("shape tree not found in slide"));
    };
    let (content_start, content_end) = element_content_bounds(&xml[sp_tree.start..sp_tree.end])?;
    let shapes = xml_direct_child_ranges(
        xml,
        sp_tree.start + content_start,
        sp_tree.start + content_end,
    )?;
    for shape in shapes.into_iter().filter(|shape| shape.kind == "sp") {
        let fragment = &xml[shape.start..shape.end];
        if first_c_nv_pr_id(fragment) == Some(shape_id) {
            return Ok(Some(XmlSpan {
                start: shape.start,
                end: shape.end,
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

fn direct_child_range(fragment: &str, wanted: &str) -> CliResult<Option<crate::XmlNamedRange>> {
    let (content_start, content_end) = element_content_bounds(fragment)?;
    Ok(
        xml_direct_child_ranges(fragment, content_start, content_end)?
            .into_iter()
            .find(|child| child.kind == wanted),
    )
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

fn first_element_attrs(fragment: &str) -> CliResult<BTreeMap<String, String>> {
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let mut attrs = BTreeMap::new();
                for attr in e.attributes().with_checks(false).flatten() {
                    attrs.insert(
                        local_name(attr.key.as_ref()).to_string(),
                        String::from_utf8_lossy(attr.value.as_ref()).to_string(),
                    );
                }
                return Ok(attrs);
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(BTreeMap::new())
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

fn expand_self_closing(fragment: &str) -> CliResult<String> {
    let open_end = fragment
        .find('>')
        .ok_or_else(|| CliError::unexpected("invalid PPTX XML"))?;
    if !fragment[..=open_end].trim_end().ends_with("/>") {
        return Ok(fragment.to_string());
    }
    let open_tag = &fragment[..=open_end];
    let slash_index = open_tag
        .rfind('/')
        .ok_or_else(|| CliError::unexpected("invalid self-closing PPTX XML"))?;
    let start_tag = open_tag[..slash_index].trim_end();
    let tag_name = start_tag
        .trim_start()
        .strip_prefix('<')
        .and_then(|name| name.split_whitespace().next())
        .ok_or_else(|| CliError::unexpected("invalid self-closing PPTX XML"))?;
    Ok(format!("{start_tag}></{tag_name}>"))
}

fn insert_xml_at(xml: &str, index: usize, insert: &str) -> String {
    let mut out = String::with_capacity(xml.len() + insert.len());
    out.push_str(&xml[..index]);
    out.push_str(insert);
    out.push_str(&xml[index..]);
    out
}

fn replace_xml_span(xml: &str, start: usize, end: usize, replacement: &str) -> String {
    let mut out = String::with_capacity(xml.len() - (end - start) + replacement.len());
    out.push_str(&xml[..start]);
    out.push_str(replacement);
    out.push_str(&xml[end..]);
    out
}
