mod image_batch;
mod image_payload;
mod output;
mod text_xlsx;

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use self::image_batch::{
    ImageBatchSelector, parse_image_batch_selector, parse_image_batch_slide_spec,
};
use self::image_payload::{
    image_content_type_from_path, normalize_fit_mode, replacement_image_uri, validate_image_payload,
};
use self::output::{
    ensure_pptx, image_batch_replace_result_json, image_replace_result_json,
    plain_text_result_json, render_relationships_xml, sha256_string, text_occurrences_result_json,
    write_replace_mutation,
};
use self::text_xlsx::{
    ReplaceTextFromXlsxRequest, ReplaceTextMapFromXlsxRequest, TextMapApplied,
    TextTargetReplacePlan, build_text_target_replace_plan, check_expected_xlsx_source_range,
    decode_text_separator_flag, join_xlsx_text_matrix, load_xlsx_text_range_or_table_source,
    load_xlsx_text_range_source, map_text_target_error, normalize_replace_text_from_xlsx_mode,
    normalize_xlsx_formula_mode, replace_text_from_xlsx, replace_text_map_from_xlsx,
    text_map_records_from_values,
};
use crate::cli_args::value_flag_present;
use crate::{
    CliError, CliResult, append_xml_text_event, attr, attr_exact, content_type_for_part,
    ensure_content_type_override, is_xml_text_event, local_name, needs_xml_space_preserve,
    pptx_shapes_get, relationship_entries_from_xml, relationship_target_from_source_to_target,
    relationships_part_for, replace_xml_span, resolve_relationship_target,
    validate_xlsx_mutation_output_flags, xml_direct_child_ranges, xml_escape, zip_text,
};

#[derive(Clone)]
struct PptxReplaceMutationOptions {
    out: Option<String>,
    backup: Option<String>,
    dry_run: bool,
    in_place: bool,
    no_validate: bool,
}

pub(crate) fn pptx_replace_text_occurrences(file: &str, args: &[String]) -> CliResult<Value> {
    let options = parse_replace_mutation_options(args)?;
    let match_text = crate::parse_string_flag(args, "--match-text")?.unwrap_or_default();
    if !value_flag_present(args, "--match-text") || match_text.is_empty() {
        return Err(CliError::invalid_args(
            "--match-text must be specified and cannot be empty",
        ));
    }
    let new_text = resolve_text_occurrences_new_text(args)?;
    let for_slides = crate::parse_string_flag(args, "--for-slides")?.unwrap_or_default();
    let for_shape = crate::parse_string_flag(args, "--for-shape")?.unwrap_or_default();
    if !for_shape.trim().is_empty() && value_flag_present(args, "--for-slides") {
        return Err(CliError::invalid_args(
            "--for-shape and --for-slides are mutually exclusive; --for-shape already encodes its slide scope",
        ));
    }
    let expect_count = if value_flag_present(args, "--expect-count") {
        let count = crate::parse_i64_flag(args, "--expect-count")?.unwrap_or(0);
        if count < 0 {
            return Err(CliError::invalid_args("--expect-count must be >= 0"));
        }
        Some(count as usize)
    } else {
        None
    };
    let expect_plan_hash = crate::parse_string_flag(args, "--expect-plan-hash")?
        .unwrap_or_default()
        .trim()
        .to_string();
    let request = TextOccurrencesRequest {
        match_text,
        new_text,
        for_slides,
        for_shape,
        ignore_case: crate::has_flag(args, "--ignore-case"),
        expect_count,
        expect_plan_hash,
        allow_zero: crate::has_flag(args, "--allow-zero"),
    };
    replace_text_occurrences(file, request, options)
}

pub(crate) fn pptx_replace_images(file: &str, args: &[String]) -> CliResult<Value> {
    let options = parse_replace_mutation_options(args)?;
    let target = crate::parse_string_flag(args, "--target")?.unwrap_or_default();
    if target.is_empty() {
        return Err(CliError::invalid_args("--target must be specified"));
    }
    let image_file = crate::parse_string_flag(args, "--image")?.unwrap_or_default();
    if image_file.is_empty() {
        return Err(CliError::invalid_args("--image must be specified"));
    }
    let image_data = fs::read(&image_file)
        .map_err(|_| CliError::file_not_found(format!("file not found: {image_file}")))?;
    let new_content_type = image_content_type_from_path(&image_file)?;
    validate_image_payload(&image_data, &new_content_type)
        .map_err(|message| CliError::unexpected(format!("failed to replace image: {message}")))?;
    let fit_mode = normalize_fit_mode(
        crate::parse_string_flag(args, "--fit-mode")?
            .unwrap_or_else(|| "contain".to_string())
            .as_str(),
    )?;
    if value_flag_present(args, "--slide") && value_flag_present(args, "--for-slides") {
        return Err(CliError::invalid_args(
            "cannot specify both --slide and --for-slides",
        ));
    }
    if target.starts_with("H:pptx/")
        && (value_flag_present(args, "--slide") || value_flag_present(args, "--for-slides"))
    {
        return Err(CliError::invalid_args(
            "--slide / --for-slides cannot be combined with a handle target",
        ));
    }
    let for_slides = crate::parse_string_flag(args, "--for-slides")?.unwrap_or_default();
    if value_flag_present(args, "--for-slides") {
        return replace_images_for_slides(
            file,
            &target,
            &for_slides,
            &image_data,
            &new_content_type,
            &fit_mode,
            options,
        );
    }
    let slide = if value_flag_present(args, "--slide") {
        let slide = crate::parse_i64_flag(args, "--slide")?.unwrap_or(0);
        if slide < 1 {
            return Err(CliError::invalid_args("--slide must be >= 1"));
        }
        Some(slide as u32)
    } else {
        None
    };
    replace_image(
        file,
        &target,
        slide,
        &image_data,
        &new_content_type,
        &fit_mode,
        options,
    )
}

pub(crate) fn pptx_replace_text(file: &str, args: &[String]) -> CliResult<Value> {
    let slide = crate::parse_i64_flag(args, "--slide")?.unwrap_or(1);
    if slide < 1 {
        return Err(CliError::invalid_args("--slide must be >= 1"));
    }
    let target = crate::parse_string_flag(args, "--target")?
        .ok_or_else(|| CliError::invalid_args("--target is required"))?;
    if target.trim().is_empty() {
        return Err(CliError::invalid_args("--target is required"));
    }
    let text = crate::parse_string_flag(args, "--text")?
        .ok_or_else(|| CliError::invalid_args("--text is required"))?;
    let out = crate::parse_string_flag(args, "--out")?
        .ok_or_else(|| CliError::invalid_args("--out is required"))?;
    replace_plain_text(
        file,
        slide as u32,
        &target,
        &text,
        PptxReplaceMutationOptions {
            out: Some(out),
            backup: None,
            dry_run: false,
            in_place: false,
            no_validate: false,
        },
    )
}

pub(crate) fn pptx_replace_text_in_place(
    file: &str,
    slide: u32,
    target: &str,
    text: &str,
) -> CliResult<Value> {
    replace_plain_text(
        file,
        slide,
        target,
        text,
        PptxReplaceMutationOptions {
            out: None,
            backup: None,
            dry_run: false,
            in_place: true,
            no_validate: true,
        },
    )
}

fn replace_plain_text(
    file: &str,
    slide: u32,
    target: &str,
    text: &str,
    options: PptxReplaceMutationOptions,
) -> CliResult<Value> {
    ensure_pptx(file)?;
    let slides = pptx_slide_refs_for_replace(file)?;
    let plan = build_text_target_replace_plan(file, &slides, slide, target, text)
        .map_err(|err| map_text_target_error(err, target))?;
    let mut text_overrides = BTreeMap::new();
    text_overrides.insert(plan.slide_part.clone(), plan.slide_xml.clone());
    write_replace_mutation(file, &text_overrides, &BTreeMap::new(), &options)?;
    let mut result = plain_text_result_json(file, target, &plan, &options);
    if let Some(readback_file) = plain_text_output_path(file, &options) {
        let destination = plain_text_readback_destination(
            &readback_file,
            plan.slide,
            target,
            &plan.target.primary_selector,
        )?;
        if let Some(object) = result.as_object_mut() {
            object.insert("destination".to_string(), destination);
        }
    }
    Ok(result)
}

fn plain_text_output_path(file: &str, options: &PptxReplaceMutationOptions) -> Option<String> {
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

fn plain_text_readback_destination(
    readback_file: &str,
    slide: u32,
    requested_target: &str,
    readback_target: &str,
) -> CliResult<Value> {
    let get = pptx_shapes_get(readback_file, slide, readback_target, true, true)?;
    let entry = get
        .get("shapes")
        .and_then(Value::as_array)
        .and_then(|shapes| shapes.first())
        .ok_or_else(|| CliError::unexpected("shape readback missing destination"))?;
    let mut out = Map::new();
    out.insert("file".to_string(), json!(readback_file));
    out.insert("slide".to_string(), json!(slide));
    out.insert("target".to_string(), json!(requested_target));
    for key in [
        "shapeId",
        "shapeName",
        "targetKind",
        "primarySelector",
        "handle",
        "selectors",
        "textPreview",
        "bounds",
    ] {
        if let Some(value) = entry.get(key) {
            out.insert(key.to_string(), value.clone());
        }
    }
    Ok(Value::Object(out))
}

pub(crate) fn pptx_replace_text_from_xlsx(file: &str, args: &[String]) -> CliResult<Value> {
    let slide = crate::parse_i64_flag(args, "--slide")?.unwrap_or(0);
    if slide < 1 {
        return Err(CliError::invalid_args("--slide must be >= 1"));
    }
    let target = crate::parse_string_flag(args, "--target")?.unwrap_or_default();
    if target.trim().is_empty() {
        return Err(CliError::invalid_args("--target is required"));
    }
    let workbook = crate::parse_string_flag(args, "--workbook")?.unwrap_or_default();
    if workbook.trim().is_empty() {
        return Err(CliError::invalid_args("--workbook is required"));
    }
    if !Path::new(&workbook).exists() {
        return Err(CliError::file_not_found(format!(
            "file not found: {workbook}"
        )));
    }
    let sheet = crate::parse_string_flag(args, "--sheet")?.unwrap_or_default();
    if sheet.trim().is_empty() {
        return Err(CliError::invalid_args("--sheet is required"));
    }
    let range = crate::parse_string_flag(args, "--range")?.unwrap_or_default();
    if range.trim().is_empty() {
        return Err(CliError::invalid_args("--range is required"));
    }
    let max_cells = crate::parse_i64_flag(args, "--max-cells")?.unwrap_or(100000);
    let formula_mode = normalize_xlsx_formula_mode(
        crate::parse_string_flag(args, "--formula-mode")?.as_deref(),
        "--formula-mode",
    )?;
    let mode = normalize_replace_text_from_xlsx_mode(
        crate::parse_string_flag(args, "--mode")?.as_deref(),
    )?;
    let row_sep = decode_text_separator_flag(
        crate::parse_string_flag(args, "--row-sep")?
            .unwrap_or_else(|| "\n".to_string())
            .as_str(),
        "--row-sep",
    )?;
    let col_sep = decode_text_separator_flag(
        crate::parse_string_flag(args, "--col-sep")?
            .unwrap_or_else(|| "\t".to_string())
            .as_str(),
        "--col-sep",
    )?;
    let source = load_xlsx_text_range_source(&workbook, &sheet, &range, max_cells, &formula_mode)?;
    let text = join_xlsx_text_matrix(&source.data, &row_sep, &col_sep);
    let options = parse_replace_mutation_options(args)?;
    replace_text_from_xlsx(
        file,
        ReplaceTextFromXlsxRequest {
            slide: slide as u32,
            target,
            source,
            text,
            mode,
            formula_mode,
            row_sep,
            col_sep,
        },
        options,
    )
}

pub(crate) fn pptx_replace_text_map_from_xlsx(file: &str, args: &[String]) -> CliResult<Value> {
    let workbook = crate::parse_string_flag(args, "--workbook")?.unwrap_or_default();
    if workbook.trim().is_empty() {
        return Err(CliError::invalid_args("--workbook is required"));
    }
    if !Path::new(&workbook).exists() {
        return Err(CliError::file_not_found(format!(
            "file not found: {workbook}"
        )));
    }
    let max_cells = crate::parse_i64_flag(args, "--max-cells")?.unwrap_or(100000);
    let formula_mode = normalize_xlsx_formula_mode(
        crate::parse_string_flag(args, "--formula-mode")?.as_deref(),
        "--formula-mode",
    )?;
    let mode = normalize_replace_text_from_xlsx_mode(
        crate::parse_string_flag(args, "--mode")?.as_deref(),
    )?;
    let source = load_xlsx_text_range_or_table_source(
        &workbook,
        crate::parse_string_flag(args, "--sheet")?.as_deref(),
        crate::parse_string_flag(args, "--range")?.as_deref(),
        crate::parse_string_flag(args, "--table")?.as_deref(),
        max_cells,
        &formula_mode,
    )?;
    check_expected_xlsx_source_range(
        &source.range,
        crate::parse_string_flag(args, "--expect-source-range")?.as_deref(),
    )?;
    let (records, columns) = text_map_records_from_values(
        &source.data,
        crate::parse_string_flag(args, "--slide-col")?
            .unwrap_or_else(|| "slide".to_string())
            .as_str(),
        crate::parse_string_flag(args, "--target-col")?
            .unwrap_or_else(|| "target".to_string())
            .as_str(),
        crate::parse_string_flag(args, "--text-col")?
            .unwrap_or_else(|| "text".to_string())
            .as_str(),
    )?;
    let options = parse_replace_mutation_options(args)?;
    replace_text_map_from_xlsx(
        file,
        ReplaceTextMapFromXlsxRequest {
            source,
            records,
            columns,
            mode,
            formula_mode,
        },
        options,
    )
}

fn parse_replace_mutation_options(args: &[String]) -> CliResult<PptxReplaceMutationOptions> {
    let out = crate::parse_string_flag(args, "--out")?;
    let backup = crate::parse_string_flag(args, "--backup")?;
    let dry_run = crate::has_flag(args, "--dry-run");
    let in_place = crate::has_flag(args, "--in-place");
    let no_validate = crate::has_flag(args, "--no-validate");
    validate_xlsx_mutation_output_flags(out.as_deref(), in_place, backup.as_deref(), dry_run)?;
    Ok(PptxReplaceMutationOptions {
        out,
        backup,
        dry_run,
        in_place,
        no_validate,
    })
}

fn resolve_text_occurrences_new_text(args: &[String]) -> CliResult<String> {
    let has_inline = value_flag_present(args, "--new-text");
    let has_file = value_flag_present(args, "--new-text-file");
    if has_inline == has_file {
        return Err(CliError::invalid_args(
            "must specify exactly one of --new-text or --new-text-file",
        ));
    }
    if has_inline {
        return Ok(crate::parse_string_flag(args, "--new-text")?.unwrap_or_default());
    }
    let path = crate::parse_string_flag(args, "--new-text-file")?.unwrap_or_default();
    fs::read_to_string(&path)
        .map_err(|_| CliError::file_not_found(format!("file not found: {path}")))
}

struct TextOccurrencesRequest {
    match_text: String,
    new_text: String,
    for_slides: String,
    for_shape: String,
    ignore_case: bool,
    expect_count: Option<usize>,
    expect_plan_hash: String,
    allow_zero: bool,
}

struct TextOccurrencePlan {
    selected_slides: Vec<u32>,
    slides_scanned: usize,
    targets_scanned: usize,
    text_nodes_scanned: usize,
    changed_target_count: usize,
    replacement_count: usize,
    plan_hash: String,
    matches: Vec<TextOccurrenceMatch>,
    overrides: BTreeMap<String, String>,
    shape_scoped: bool,
}

#[derive(Clone)]
struct TextOccurrenceMatch {
    slide_number: u32,
    part_uri: String,
    shape_id: u32,
    shape_name: String,
    target_kind: String,
    primary_selector: String,
    selectors: Vec<String>,
    text_node_index: usize,
    match_count: usize,
    before_text: String,
    after_text: String,
    source_hash: String,
}

fn replace_text_occurrences(
    file: &str,
    request: TextOccurrencesRequest,
    options: PptxReplaceMutationOptions,
) -> CliResult<Value> {
    ensure_pptx(file)?;
    let slides = pptx_slide_refs_for_replace(file)?;
    let shape_scope = resolve_shape_scope(&request.for_shape, &slides)?;
    let selected_slides = if let Some((slide_number, _)) = shape_scope {
        vec![slide_number]
    } else {
        resolve_slide_selection(&request.for_slides, &slides)?
    };
    let mut plan =
        build_text_occurrence_plan(file, &slides, &selected_slides, shape_scope, &request)?;
    if let Some(expected) = request.expect_count
        && expected != plan.replacement_count
    {
        return Err(CliError::invalid_args(format!(
            "text occurrences guard mismatch: --expect-count is {expected} but planned replacements are {}",
            plan.replacement_count
        )));
    }
    if !request.expect_plan_hash.is_empty() && request.expect_plan_hash != plan.plan_hash {
        return Err(CliError::invalid_args(format!(
            "text occurrences guard mismatch: --expect-plan-hash is {} but current plan hash is {}",
            request.expect_plan_hash, plan.plan_hash
        )));
    }
    if plan.replacement_count == 0 && !options.dry_run && !request.allow_zero {
        return Err(CliError::invalid_args(
            "text occurrences no matches: no occurrences of match text were found",
        ));
    }
    write_replace_mutation(file, &plan.overrides, &BTreeMap::new(), &options)?;
    Ok(text_occurrences_result_json(
        file, &request, &mut plan, &options,
    ))
}

fn build_text_occurrence_plan(
    file: &str,
    slides: &[PptxSlideRef],
    selected_slides: &[u32],
    shape_scope: Option<(u32, u32)>,
    request: &TextOccurrencesRequest,
) -> CliResult<TextOccurrencePlan> {
    let mut plan = TextOccurrencePlan {
        selected_slides: selected_slides.to_vec(),
        slides_scanned: 0,
        targets_scanned: 0,
        text_nodes_scanned: 0,
        changed_target_count: 0,
        replacement_count: 0,
        plan_hash: String::new(),
        matches: Vec::new(),
        overrides: BTreeMap::new(),
        shape_scoped: shape_scope.is_some(),
    };
    let mut changed_targets = BTreeSet::<String>::new();

    for slide_number in selected_slides {
        let slide_ref = slides
            .get(*slide_number as usize - 1)
            .ok_or_else(|| CliError::unexpected(format!("slide {slide_number} not found")))?;
        let slide_xml = zip_text(file, &slide_ref.part)?;
        let targets = slide_targets(&slide_xml, slide_ref);
        let mut replacements = Vec::<TextNodeReplacement>::new();
        plan.slides_scanned += 1;
        for target in targets {
            if let Some((scope_slide, scope_shape)) = shape_scope
                && (scope_slide != *slide_number || scope_shape != target.shape_id)
            {
                continue;
            }
            let text_nodes = text_nodes_in_span(&slide_xml, target.span)?;
            if text_nodes.is_empty() {
                continue;
            }
            plan.targets_scanned += 1;
            for (node_index, node) in text_nodes.iter().enumerate() {
                plan.text_nodes_scanned += 1;
                let (after, count) = replace_text_occurrences_in_string(
                    &node.before_text,
                    &request.match_text,
                    &request.new_text,
                    request.ignore_case,
                );
                if count == 0 {
                    continue;
                }
                plan.replacement_count += count;
                changed_targets.insert(format!("{slide_number}:{}", target.shape_id));
                plan.matches.push(TextOccurrenceMatch {
                    slide_number: *slide_number,
                    part_uri: format!("/{}", slide_ref.part),
                    shape_id: target.shape_id,
                    shape_name: target.shape_name.clone(),
                    target_kind: target.target_kind.clone(),
                    primary_selector: target.primary_selector.clone(),
                    selectors: target.selectors.clone(),
                    text_node_index: node_index + 1,
                    match_count: count,
                    before_text: node.before_text.clone(),
                    after_text: after.clone(),
                    source_hash: sha256_string(&node.before_text),
                });
                replacements.push(TextNodeReplacement {
                    span: node.clone(),
                    after,
                });
            }
        }
        if !replacements.is_empty() {
            plan.overrides.insert(
                slide_ref.part.clone(),
                apply_text_node_replacements(&slide_xml, &mut replacements),
            );
        }
    }

    plan.changed_target_count = changed_targets.len();
    plan.plan_hash = compute_text_occurrences_plan_hash(
        &request.match_text,
        &request.new_text,
        request.ignore_case,
        &plan,
    );
    Ok(plan)
}

fn resolve_shape_scope(value: &str, slides: &[PptxSlideRef]) -> CliResult<Option<(u32, u32)>> {
    if value.trim().is_empty() {
        return Ok(None);
    }
    let Some(rest) = value.strip_prefix("H:pptx/s:") else {
        return Err(CliError::invalid_args(
            "--for-shape must be a shape handle (H:pptx/s:<sldId>/shape:n:<id>)",
        ));
    };
    let Some((slide_id_raw, shape_raw)) = rest.split_once("/shape:n:") else {
        return Err(CliError::invalid_args(
            "--for-shape must be a shape handle (H:pptx/s:<sldId>/shape:n:<id>)",
        ));
    };
    let slide_id = slide_id_raw
        .parse::<u32>()
        .map_err(|_| CliError::invalid_args("malformed PPTX shape handle"))?;
    let shape_id = shape_raw
        .parse::<u32>()
        .map_err(|_| CliError::invalid_args("malformed PPTX shape handle"))?;
    let slide_number = slides
        .iter()
        .find(|slide| slide.slide_id == slide_id)
        .map(|slide| slide.number)
        .ok_or_else(|| {
            CliError::target_not_found(format!(
                "stale PPTX handle: slide sldId {slide_id} not found"
            ))
        })?;
    Ok(Some((slide_number, shape_id)))
}

fn resolve_slide_selection(value: &str, slides: &[PptxSlideRef]) -> CliResult<Vec<u32>> {
    if value.trim().is_empty() {
        return Ok(slides.iter().map(|slide| slide.number).collect());
    }
    if let Some(raw) = value.strip_prefix("H:pptx/s:") {
        let slide_id = raw
            .parse::<u32>()
            .map_err(|_| CliError::invalid_args("malformed PPTX slide handle"))?;
        let slide = slides
            .iter()
            .find(|slide| slide.slide_id == slide_id)
            .ok_or_else(|| {
                CliError::target_not_found(format!(
                    "stale PPTX handle: slide sldId {slide_id} not found"
                ))
            })?;
        return Ok(vec![slide.number]);
    }
    let parsed = parse_slide_spec(value).map_err(|message| {
        CliError::invalid_args(format!("invalid slide specification: {message}"))
    })?;
    if parsed.is_empty() {
        return Err(CliError::invalid_args(
            "no valid slides specified in --for-slides",
        ));
    }
    let mut out = Vec::new();
    for slide_number in parsed {
        if slide_number < 1 || slide_number as usize > slides.len() {
            return Err(CliError::invalid_args(format!(
                "slide {slide_number} not found (presentation has {} slides)",
                slides.len()
            )));
        }
        if !out.contains(&slide_number) {
            out.push(slide_number);
        }
    }
    Ok(out)
}

fn parse_slide_spec(value: &str) -> Result<Vec<u32>, String> {
    let mut slides = Vec::new();
    for part in value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        if let Some((start, end)) = part.split_once('-') {
            let start = start
                .trim()
                .parse::<u32>()
                .map_err(|_| format!("invalid slide number {part:?}"))?;
            let end = end
                .trim()
                .parse::<u32>()
                .map_err(|_| format!("invalid slide number {part:?}"))?;
            if start > end {
                return Err(format!("invalid slide range {part:?}"));
            }
            slides.extend(start..=end);
        } else {
            slides.push(
                part.parse::<u32>()
                    .map_err(|_| format!("invalid slide number {part:?}"))?,
            );
        }
    }
    Ok(slides)
}

fn replace_text_occurrences_in_string(
    text: &str,
    match_text: &str,
    replacement: &str,
    ignore_case: bool,
) -> (String, usize) {
    if text.is_empty() || match_text.is_empty() {
        return (text.to_string(), 0);
    }
    if !ignore_case {
        let count = text.matches(match_text).count();
        if count == 0 {
            return (text.to_string(), 0);
        }
        return (text.replace(match_text, replacement), count);
    }
    let haystack = text.to_ascii_lowercase();
    let needle = match_text.to_ascii_lowercase();
    let mut out = String::new();
    let mut offset = 0;
    let mut count = 0;
    while let Some(found) = haystack[offset..].find(&needle) {
        let start = offset + found;
        let end = start + match_text.len();
        out.push_str(&text[offset..start]);
        out.push_str(replacement);
        offset = end;
        count += 1;
    }
    if count == 0 {
        return (text.to_string(), 0);
    }
    out.push_str(&text[offset..]);
    (out, count)
}

fn apply_text_node_replacements(xml: &str, replacements: &mut [TextNodeReplacement]) -> String {
    replacements.sort_by_key(|replacement| replacement.span.content_start);
    let mut updated = xml.to_string();
    for replacement in replacements.iter().rev() {
        updated = replace_xml_span(
            &updated,
            replacement.span.content_start,
            replacement.span.content_end,
            &xml_escape(&replacement.after),
        );
        if needs_xml_space_preserve(&replacement.after) && !replacement.span.has_xml_space {
            updated = replace_xml_span(
                &updated,
                replacement.span.open_end - 1,
                replacement.span.open_end - 1,
                r#" xml:space="preserve""#,
            );
        }
    }
    updated
}

fn compute_text_occurrences_plan_hash(
    match_text: &str,
    new_text: &str,
    ignore_case: bool,
    plan: &TextOccurrencePlan,
) -> String {
    let mut hasher = Sha256::new();
    let mut write_field = |value: &str| {
        hasher.update(value.as_bytes());
        hasher.update([0]);
    };
    write_field("pptx.replace.text-occurrences");
    write_field(match_text);
    write_field(new_text);
    write_field(if ignore_case { "true" } else { "false" });
    for slide in &plan.selected_slides {
        write_field(&slide.to_string());
    }
    for item in &plan.matches {
        write_field(&item.slide_number.to_string());
        write_field(&item.part_uri);
        write_field(&item.shape_id.to_string());
        write_field(&item.primary_selector);
        write_field(&item.text_node_index.to_string());
        write_field(&item.match_count.to_string());
        write_field(&item.before_text);
        write_field(&item.source_hash);
    }
    format!("sha256:{:x}", hasher.finalize())
}

struct ImageReplacePlan {
    slide: u32,
    target: ShapeTarget,
    slide_xml: String,
    rels_part: String,
    rels_xml: String,
    old_target_uri: String,
    old_content_type: String,
    new_target_uri: String,
    new_content_type: String,
    relationship_id: String,
}

struct ImageBatchReplacePlan {
    slides: Vec<ImageBatchSlideResult>,
    text_overrides: BTreeMap<String, String>,
    binary_overrides: BTreeMap<String, Vec<u8>>,
    success_count: usize,
    not_found_count: usize,
    error_count: usize,
}

struct ImageBatchSlideResult {
    slide: u32,
    success: bool,
    not_found: bool,
    error: String,
    plan: Option<ImageReplacePlan>,
}

fn replace_image(
    file: &str,
    target_selector: &str,
    slide: Option<u32>,
    image_data: &[u8],
    new_content_type: &str,
    fit_mode: &str,
    options: PptxReplaceMutationOptions,
) -> CliResult<Value> {
    ensure_pptx(file)?;
    let slides = pptx_slide_refs_for_replace(file)?;
    if let Some(slide) = slide
        && (slide < 1 || slide as usize > slides.len())
    {
        return Err(CliError::unexpected(format!(
            "failed to replace image: invalid slide number {slide} (presentation has {} slides)",
            slides.len()
        )));
    }
    let plan = plan_image_replace(
        file,
        &slides,
        target_selector,
        slide,
        new_content_type,
        fit_mode,
    )?;
    let mut text_overrides = BTreeMap::new();
    text_overrides.insert(
        slides[plan.slide as usize - 1].part.clone(),
        plan.slide_xml.clone(),
    );
    text_overrides.insert(plan.rels_part.clone(), plan.rels_xml.clone());
    let mut content_types = zip_text(file, "[Content_Types].xml")?;
    content_types =
        ensure_content_type_override(content_types, &plan.new_target_uri, &plan.new_content_type)?;
    text_overrides.insert("[Content_Types].xml".to_string(), content_types);
    let mut binary_overrides = BTreeMap::new();
    binary_overrides.insert(
        plan.new_target_uri.trim_start_matches('/').to_string(),
        image_data.to_vec(),
    );
    write_replace_mutation(file, &text_overrides, &binary_overrides, &options)?;
    Ok(image_replace_result_json(
        file,
        target_selector,
        fit_mode,
        &plan,
        &options,
    ))
}

fn plan_image_replace(
    file: &str,
    slides: &[PptxSlideRef],
    target_selector: &str,
    requested_slide: Option<u32>,
    new_content_type: &str,
    fit_mode: &str,
) -> CliResult<ImageReplacePlan> {
    for slide_ref in slides {
        if let Some(requested_slide) = requested_slide
            && requested_slide != slide_ref.number
        {
            continue;
        }
        let slide_xml = zip_text(file, &slide_ref.part)?;
        let targets = slide_targets(&slide_xml, slide_ref);
        let Some(target) = targets
            .iter()
            .find(|target| {
                target.target_kind == "picture" && target.matches_selector(target_selector)
            })
            .cloned()
        else {
            continue;
        };
        let relationship_id = target.image_rel_id.clone();
        if relationship_id.is_empty() {
            return Err(CliError::unexpected(
                "failed to replace image: no relationship ID found in blip element",
            ));
        }
        let rels_part = relationships_part_for(&slide_ref.part);
        let rels_xml = zip_text(file, &rels_part)?;
        let mut rels = relationship_entries_from_xml(&rels_xml);
        let Some(rel_index) = rels.iter().position(|rel| rel.id == relationship_id) else {
            return Err(CliError::unexpected(format!(
                "failed to replace image: could not resolve relationship {relationship_id}",
            )));
        };
        if rels[rel_index].target_mode == "External" {
            return Err(CliError::target_not_found(format!(
                "picture shape not found: {target_selector}"
            )));
        }
        let old_target_uri =
            resolve_relationship_target(&format!("/{}", slide_ref.part), &rels[rel_index].target);
        let old_content_type = content_type_for_part(file, &old_target_uri).unwrap_or_default();
        let new_target_uri =
            replacement_image_uri(&old_target_uri, &old_content_type, new_content_type)?;
        if new_target_uri != old_target_uri {
            rels[rel_index].target = relationship_target_from_source_to_target(
                &format!("/{}", slide_ref.part),
                &new_target_uri,
            );
        }
        let rels_xml = render_relationships_xml(&rels);
        let slide_xml = update_picture_fit_mode(&slide_xml, target.span, fit_mode)?;
        return Ok(ImageReplacePlan {
            slide: slide_ref.number,
            target,
            slide_xml,
            rels_part,
            rels_xml,
            old_target_uri,
            old_content_type,
            new_target_uri,
            new_content_type: new_content_type.to_string(),
            relationship_id,
        });
    }
    if let Some(slide) = requested_slide {
        return Err(image_target_not_found(target_selector, slide, file));
    }
    Err(CliError::target_not_found(format!(
        "picture shape not found: {target_selector}; discover with `ooxml --json pptx slides show <file> --include-bounds`"
    )))
}

fn replace_images_for_slides(
    file: &str,
    target_selector: &str,
    for_slides: &str,
    image_data: &[u8],
    new_content_type: &str,
    fit_mode: &str,
    options: PptxReplaceMutationOptions,
) -> CliResult<Value> {
    ensure_pptx(file)?;
    let selector = parse_image_batch_selector(target_selector)?;
    let selected_slides = parse_image_batch_slide_spec(for_slides).map_err(|message| {
        CliError::invalid_args(format!("invalid slide specification: {message}"))
    })?;
    if selected_slides.is_empty() {
        return Err(CliError::invalid_args(
            "no valid slides specified in --for-slides",
        ));
    }
    let slides = pptx_slide_refs_for_replace(file)?;
    let plan = plan_image_batch_replace(
        file,
        &slides,
        &selected_slides,
        &selector,
        image_data,
        new_content_type,
        fit_mode,
    )?;
    write_replace_mutation(file, &plan.text_overrides, &plan.binary_overrides, &options)?;
    Ok(image_batch_replace_result_json(&selector.normalized, &plan))
}

fn plan_image_batch_replace(
    file: &str,
    slides: &[PptxSlideRef],
    selected_slides: &[u32],
    selector: &ImageBatchSelector,
    image_data: &[u8],
    new_content_type: &str,
    fit_mode: &str,
) -> CliResult<ImageBatchReplacePlan> {
    let mut plan = ImageBatchReplacePlan {
        slides: Vec::with_capacity(selected_slides.len()),
        text_overrides: BTreeMap::new(),
        binary_overrides: BTreeMap::new(),
        success_count: 0,
        not_found_count: 0,
        error_count: 0,
    };

    for &slide in selected_slides {
        if slide < 1 || slide as usize > slides.len() {
            plan.not_found_count += 1;
            plan.slides.push(ImageBatchSlideResult {
                slide,
                success: false,
                not_found: true,
                error: format!(
                    "slide {slide} not found (presentation has {} slides)",
                    slides.len()
                ),
                plan: None,
            });
            continue;
        }
        if let Some(message) = selector.unsupported_error.as_deref() {
            plan.error_count += 1;
            plan.slides.push(ImageBatchSlideResult {
                slide,
                success: false,
                not_found: false,
                error: message.to_string(),
                plan: None,
            });
            continue;
        }
        match plan_image_replace(
            file,
            slides,
            &selector.normalized,
            Some(slide),
            new_content_type,
            fit_mode,
        ) {
            Ok(replace_plan) => {
                plan.success_count += 1;
                plan.text_overrides.insert(
                    slides[replace_plan.slide as usize - 1].part.clone(),
                    replace_plan.slide_xml.clone(),
                );
                plan.text_overrides.insert(
                    replace_plan.rels_part.clone(),
                    replace_plan.rels_xml.clone(),
                );
                plan.binary_overrides.insert(
                    replace_plan
                        .new_target_uri
                        .trim_start_matches('/')
                        .to_string(),
                    image_data.to_vec(),
                );
                plan.slides.push(ImageBatchSlideResult {
                    slide,
                    success: true,
                    not_found: false,
                    error: String::new(),
                    plan: Some(replace_plan),
                });
            }
            Err(err) if err.code == "target_not_found" => {
                plan.not_found_count += 1;
                plan.slides.push(ImageBatchSlideResult {
                    slide,
                    success: false,
                    not_found: true,
                    error: String::new(),
                    plan: None,
                });
            }
            Err(err) => {
                plan.error_count += 1;
                plan.slides.push(ImageBatchSlideResult {
                    slide,
                    success: false,
                    not_found: false,
                    error: err.message,
                    plan: None,
                });
            }
        }
    }

    if plan.success_count > 0
        && let Ok(content_types) = zip_text(file, "[Content_Types].xml")
    {
        let content_types = plan
            .slides
            .iter()
            .filter_map(|item| item.plan.as_ref())
            .try_fold(content_types, |content_types, item| {
                ensure_content_type_override(
                    content_types,
                    &item.new_target_uri,
                    &item.new_content_type,
                )
            })?;
        plan.text_overrides
            .insert("[Content_Types].xml".to_string(), content_types);
    }

    Ok(plan)
}

fn image_target_not_found(target_selector: &str, slide: u32, file: &str) -> CliError {
    let candidates = pptx_slide_refs_for_replace(file)
        .ok()
        .and_then(|slides| slides.get(slide as usize - 1).cloned())
        .and_then(|slide_ref| {
            zip_text(file, &slide_ref.part)
                .ok()
                .map(|xml| (slide_ref, xml))
        })
        .map(|(slide_ref, xml)| {
            slide_targets(&xml, &slide_ref)
                .into_iter()
                .filter(|target| target.target_kind == "picture")
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let hint = candidates
        .first()
        .map(|candidate| format!("; did you mean: {}", candidate.primary_selector))
        .unwrap_or_default();
    CliError::target_not_found(format!(
        "picture shape not found: {target_selector}{hint}; discover with `ooxml --json pptx shapes show <file> --slide {slide}`"
    ))
}

#[derive(Clone)]
struct PptxSlideRef {
    number: u32,
    slide_id: u32,
    part: String,
    slide_id_unique: bool,
}

fn pptx_slide_refs_for_replace(file: &str) -> CliResult<Vec<PptxSlideRef>> {
    let presentation = zip_text(file, "ppt/presentation.xml")?;
    let slide_ids = presentation_slide_refs(&presentation);
    let rels = relationship_entries_from_xml(&zip_text(file, "ppt/_rels/presentation.xml.rels")?);
    let mut id_counts = BTreeMap::<u32, usize>::new();
    for (slide_id, _) in &slide_ids {
        *id_counts.entry(*slide_id).or_default() += 1;
    }
    slide_ids
        .into_iter()
        .enumerate()
        .map(|(index, (slide_id, rel_id))| {
            let rel = rels
                .iter()
                .find(|candidate| candidate.id == rel_id)
                .ok_or_else(|| CliError::unexpected(format!("missing relationship {rel_id}")))?;
            let part = resolve_relationship_target("/ppt/presentation.xml", &rel.target)
                .trim_start_matches('/')
                .to_string();
            Ok(PptxSlideRef {
                number: index as u32 + 1,
                slide_id,
                part,
                slide_id_unique: id_counts.get(&slide_id).copied().unwrap_or_default() == 1,
            })
        })
        .collect()
}

fn presentation_slide_refs(xml: &str) -> Vec<(u32, String)> {
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

#[derive(Clone, Copy)]
struct XmlSpan {
    start: usize,
    end: usize,
}

#[derive(Clone, Copy)]
struct Bounds {
    x: i64,
    y: i64,
    cx: i64,
    cy: i64,
}

#[derive(Clone)]
struct ShapeTarget {
    shape_id: u32,
    shape_name: String,
    target_kind: String,
    primary_selector: String,
    selectors: Vec<String>,
    handle: String,
    bounds: Option<Bounds>,
    image_rel_id: String,
    span: XmlSpan,
}

impl ShapeTarget {
    fn matches_selector(&self, selector: &str) -> bool {
        self.primary_selector == selector
            || self.handle == selector
            || self.selectors.iter().any(|item| item == selector)
    }
}

#[derive(Clone)]
struct ShapeScan {
    shape_id: u32,
    shape_name: String,
    shape_type: String,
    is_placeholder: bool,
    placeholder_type: String,
    placeholder_index: Option<u32>,
    has_text_body: bool,
    has_table: bool,
    image_rel_id: String,
    bounds: Option<Bounds>,
    start: usize,
    depth_or_end: usize,
}

fn slide_targets(xml: &str, slide_ref: &PptxSlideRef) -> Vec<ShapeTarget> {
    let scans = shape_scans(xml);
    let mut name_counts = BTreeMap::<String, usize>::new();
    let mut index_counts = BTreeMap::<u32, usize>::new();
    let mut shape_id_counts = BTreeMap::<u32, usize>::new();
    for scan in &scans {
        if !scan.shape_name.trim().is_empty() {
            *name_counts.entry(scan.shape_name.clone()).or_default() += 1;
        }
        if let Some(index) = scan.placeholder_index {
            *index_counts.entry(index).or_default() += 1;
        }
        *shape_id_counts.entry(scan.shape_id).or_default() += 1;
    }
    let mut table_index = 0_u32;
    scans
        .into_iter()
        .map(|scan| {
            let is_table = scan.shape_type == "graphicFrame" && scan.has_table;
            if is_table {
                table_index += 1;
            }
            let mut placeholder_role = placeholder_role(&scan.placeholder_type);
            let mut placeholder_key = placeholder_role.clone();
            if placeholder_role.is_empty()
                && scan.shape_type == "sp"
                && scan.has_text_body
                && scan
                    .shape_name
                    .to_ascii_lowercase()
                    .contains("content placeholder")
            {
                placeholder_role = "body".to_string();
                placeholder_key = "body".to_string();
            }
            let primary_selector = if is_table {
                format!("table:{table_index}")
            } else if !placeholder_key.is_empty() {
                placeholder_key.clone()
            } else {
                format!("shape:{}", scan.shape_id)
            };
            let mut selectors = Vec::new();
            if is_table {
                add_selector(&mut selectors, format!("shape:{}", scan.shape_id));
                add_selector(&mut selectors, format!("table:{table_index}"));
            } else {
                add_selector(&mut selectors, placeholder_key.clone());
                if !placeholder_role.is_empty() {
                    add_selector(&mut selectors, format!("@{placeholder_role}"));
                    add_selector(&mut selectors, placeholder_role.clone());
                    if let Some(index) = scan.placeholder_index {
                        add_selector(&mut selectors, format!("{placeholder_role}:{index}"));
                    }
                }
                if let Some(index) = scan.placeholder_index
                    && index_counts.get(&index).copied().unwrap_or_default() == 1
                {
                    add_selector(&mut selectors, format!("#{index}"));
                }
                add_selector(&mut selectors, format!("shape:{}", scan.shape_id));
            }
            if name_counts
                .get(&scan.shape_name)
                .copied()
                .unwrap_or_default()
                == 1
            {
                add_selector(&mut selectors, format!("~{}", scan.shape_name));
            }
            let target_kind = if is_table {
                "table".to_string()
            } else if scan.shape_type == "pic" {
                "picture".to_string()
            } else if !placeholder_role.is_empty() {
                placeholder_role
            } else if scan.has_text_body {
                "textbox".to_string()
            } else if scan.is_placeholder {
                "placeholder".to_string()
            } else {
                "shape".to_string()
            };
            let handle = if slide_ref.slide_id != 0
                && slide_ref.slide_id_unique
                && shape_id_counts
                    .get(&scan.shape_id)
                    .copied()
                    .unwrap_or_default()
                    == 1
            {
                format!("H:pptx/s:{}/shape:n:{}", slide_ref.slide_id, scan.shape_id)
            } else {
                String::new()
            };
            ShapeTarget {
                shape_id: scan.shape_id,
                shape_name: scan.shape_name,
                target_kind,
                primary_selector,
                selectors,
                handle,
                bounds: scan.bounds,
                image_rel_id: scan.image_rel_id,
                span: XmlSpan {
                    start: scan.start,
                    end: scan.depth_or_end,
                },
            }
        })
        .collect()
}

fn shape_scans(xml: &str) -> Vec<ShapeScan> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut path = Vec::<String>::new();
    let mut current: Option<ShapeScan> = None;
    let mut shapes = Vec::new();
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if current.is_none()
                    && path.last().map(String::as_str) == Some("spTree")
                    && matches!(name.as_str(), "sp" | "pic" | "graphicFrame")
                {
                    current = Some(ShapeScan {
                        shape_id: 0,
                        shape_name: String::new(),
                        shape_type: name.clone(),
                        is_placeholder: false,
                        placeholder_type: String::new(),
                        placeholder_index: None,
                        has_text_body: false,
                        has_table: false,
                        image_rel_id: String::new(),
                        bounds: None,
                        start: before,
                        depth_or_end: path.len() + 1,
                    });
                } else if let Some(scan) = current.as_mut() {
                    note_shape_element(scan, &e, &name);
                }
                path.push(name);
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if let Some(scan) = current.as_mut() {
                    note_shape_element(scan, &e, &name);
                }
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if let Some(scan) = current.take() {
                    if path.len() == scan.depth_or_end && name == scan.shape_type {
                        let mut finished = scan;
                        finished.depth_or_end = reader.buffer_position() as usize;
                        shapes.push(finished);
                    } else {
                        current = Some(scan);
                    }
                }
                path.pop();
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    shapes
}

fn note_shape_element(scan: &mut ShapeScan, e: &BytesStart<'_>, name: &str) {
    match name {
        "cNvPr" => {
            scan.shape_id = attr(e, "id")
                .and_then(|value| value.parse().ok())
                .unwrap_or(scan.shape_id);
            scan.shape_name = attr(e, "name").unwrap_or_else(|| scan.shape_name.clone());
        }
        "ph" => {
            scan.is_placeholder = true;
            scan.placeholder_type = attr(e, "type").unwrap_or_default();
            scan.placeholder_index = attr(e, "idx").and_then(|value| value.parse().ok());
        }
        "txBody" if scan.shape_type == "sp" => {
            scan.has_text_body = true;
        }
        "tbl" if scan.shape_type == "graphicFrame" => {
            scan.has_table = true;
        }
        "blip" if scan.shape_type == "pic" => {
            scan.image_rel_id = attr(e, "embed").unwrap_or_default();
        }
        "off" => {
            let mut bounds = scan.bounds.unwrap_or(Bounds {
                x: 0,
                y: 0,
                cx: 0,
                cy: 0,
            });
            bounds.x = attr(e, "x")
                .and_then(|value| value.parse().ok())
                .unwrap_or(bounds.x);
            bounds.y = attr(e, "y")
                .and_then(|value| value.parse().ok())
                .unwrap_or(bounds.y);
            scan.bounds = Some(bounds);
        }
        "ext" => {
            let mut bounds = scan.bounds.unwrap_or(Bounds {
                x: 0,
                y: 0,
                cx: 0,
                cy: 0,
            });
            bounds.cx = attr(e, "cx")
                .and_then(|value| value.parse().ok())
                .unwrap_or(bounds.cx);
            bounds.cy = attr(e, "cy")
                .and_then(|value| value.parse().ok())
                .unwrap_or(bounds.cy);
            scan.bounds = Some(bounds);
        }
        _ => {}
    }
}

fn placeholder_role(literal_type: &str) -> String {
    match literal_type {
        "ctrTitle" | "title" => "title",
        "subTitle" => "subtitle",
        "body" | "obj" => "body",
        "pic" => "picture",
        other => other,
    }
    .to_string()
}

fn add_selector(selectors: &mut Vec<String>, value: String) {
    if !value.is_empty() && !selectors.iter().any(|selector| selector == &value) {
        selectors.push(value);
    }
}

#[derive(Clone)]
struct TextNodeSpan {
    open_end: usize,
    content_start: usize,
    content_end: usize,
    before_text: String,
    has_xml_space: bool,
}

struct TextNodeReplacement {
    span: TextNodeSpan,
    after: String,
}

fn text_nodes_in_span(xml: &str, span: XmlSpan) -> CliResult<Vec<TextNodeSpan>> {
    let fragment = &xml[span.start..span.end];
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(false);
    let mut nodes = Vec::new();
    let mut current: Option<TextNodeScan> = None;
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if current.is_none() && name == "t" {
                    let open_end = span.start + reader.buffer_position() as usize;
                    current = Some(TextNodeScan {
                        open_end,
                        content_start: open_end,
                        content_end: open_end,
                        text: String::new(),
                        depth: 1,
                        has_xml_space: attr_exact(&e, "xml:space")
                            .or_else(|| attr(&e, "space"))
                            .is_some(),
                    });
                } else if let Some(scan) = current.as_mut() {
                    scan.depth += 1;
                }
            }
            Ok(Event::Empty(e)) => {
                if current.is_none() && local_name(e.name().as_ref()) == "t" {
                    let open_end = span.start + reader.buffer_position() as usize;
                    nodes.push(TextNodeSpan {
                        open_end,
                        content_start: open_end,
                        content_end: open_end,
                        before_text: String::new(),
                        has_xml_space: attr_exact(&e, "xml:space")
                            .or_else(|| attr(&e, "space"))
                            .is_some(),
                    });
                }
            }
            Ok(event) if is_xml_text_event(&event) => {
                if let Some(scan) = current.as_mut() {
                    append_xml_text_event(&mut scan.text, &event);
                }
            }
            Ok(Event::End(e)) => {
                if let Some(scan) = current.as_mut() {
                    if scan.depth == 1 && local_name(e.name().as_ref()) == "t" {
                        scan.content_end = span.start + before;
                        let finished = current.take().expect("text scan");
                        nodes.push(TextNodeSpan {
                            open_end: finished.open_end,
                            content_start: finished.content_start,
                            content_end: finished.content_end,
                            before_text: finished.text,
                            has_xml_space: finished.has_xml_space,
                        });
                    } else {
                        scan.depth = scan.depth.saturating_sub(1);
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(nodes)
}

struct TextNodeScan {
    open_end: usize,
    content_start: usize,
    content_end: usize,
    text: String,
    depth: usize,
    has_xml_space: bool,
}

fn update_picture_fit_mode(xml: &str, picture_span: XmlSpan, fit_mode: &str) -> CliResult<String> {
    let Some(blip_fill) = find_child_element_span(xml, picture_span, "blipFill")? else {
        return Ok(xml.to_string());
    };
    let Some((content_start, content_end)) = element_content_span(xml, blip_fill, "blipFill")?
    else {
        return Ok(xml.to_string());
    };
    let children = xml_direct_child_ranges(xml, content_start, content_end)?;
    let mut replacement = String::new();
    let mut cursor = content_start;
    for child in children {
        if child.kind == "stretch" || child.kind == "tile" {
            replacement.push_str(&xml[cursor..child.start]);
            cursor = child.end;
        }
    }
    replacement.push_str(&xml[cursor..content_end]);
    if fit_mode == "cover" {
        replacement
            .push_str(r#"<a:tile tx="0" ty="0" sx="100000" sy="100000" flip="none" algn="ctr"/>"#);
    } else {
        replacement.push_str("<a:stretch><a:fillRect/></a:stretch>");
    }
    Ok(replace_xml_span(
        xml,
        content_start,
        content_end,
        &replacement,
    ))
}

fn find_child_element_span(
    xml: &str,
    outer: XmlSpan,
    wanted_local: &str,
) -> CliResult<Option<XmlSpan>> {
    let fragment = &xml[outer.start..outer.end];
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(true);
    let mut depth = 0_usize;
    let mut found: Option<(usize, usize)> = None;
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if found.is_none() && name == wanted_local {
                    found = Some((outer.start + before, depth + 1));
                }
                depth += 1;
            }
            Ok(Event::Empty(e)) => {
                if found.is_none() && local_name(e.name().as_ref()) == wanted_local {
                    return Ok(Some(XmlSpan {
                        start: outer.start + before,
                        end: outer.start + reader.buffer_position() as usize,
                    }));
                }
            }
            Ok(Event::End(e)) => {
                if let Some((start, wanted_depth)) = found
                    && depth == wanted_depth
                    && local_name(e.name().as_ref()) == wanted_local
                {
                    return Ok(Some(XmlSpan {
                        start,
                        end: outer.start + reader.buffer_position() as usize,
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

fn element_content_span(
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
            Ok(Event::Start(_)) if depth > 0 => depth += 1,
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
