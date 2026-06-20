use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::Instant;

use crate::pptx_mutation::{pptx_clone_slide, pptx_slides_delete};
use crate::{
    CliError, CliResult, attr, copy_zip_with_part_overrides, current_utc_rfc3339, local_name,
    package_mutation_temp_path, package_type, parse_string_flag, pptx_all_slides, pptx_shapes_show,
    pptx_slides_list, reject_unknown_flags, validate, xml_escape, xml_token_name, zip_text,
};

pub(crate) fn pptx_template_capture(file: &str, args: &[String]) -> CliResult<Value> {
    reject_unknown_flags(
        args,
        &[
            "--author",
            "--description",
            "--name",
            "--organization",
            "--slides",
            "--version",
        ],
        &["--strict-shapes"],
    )?;
    if package_type(file)? != "pptx" {
        return Err(CliError::unsupported_type(
            "template capture supports PPTX/POTX files",
        ));
    }
    let name =
        parse_string_flag(args, "--name")?.unwrap_or_else(|| "Captured Template".to_string());
    let description = parse_string_flag(args, "--description")?.unwrap_or_default();
    let author = parse_string_flag(args, "--author")?.unwrap_or_default();
    let organization = parse_string_flag(args, "--organization")?.unwrap_or_default();
    let version = parse_version(
        parse_string_flag(args, "--version")?
            .as_deref()
            .unwrap_or("1.0.0"),
    )?;
    let slides = parse_capture_slides(parse_string_flag(args, "--slides")?.as_deref(), file)?;
    let timestamp = current_utc_rfc3339();
    let slide_list = pptx_slides_list(file)?;
    let slide_items = slide_list
        .get("slides")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut archetypes = Vec::<Value>::new();
    for slide in slides {
        let slide_meta = slide_items
            .iter()
            .find(|item| item.get("number").and_then(Value::as_u64) == Some(u64::from(slide)))
            .cloned()
            .unwrap_or_else(|| json!({}));
        archetypes.push(capture_archetype(file, slide, &slide_meta)?);
    }
    let mut version_object = Map::new();
    version_object.insert("major".to_string(), json!(version.0));
    version_object.insert("minor".to_string(), json!(version.1));
    version_object.insert("patch".to_string(), json!(version.2));
    version_object.insert("createdAt".to_string(), json!(timestamp));

    let mut manifest = Map::new();
    manifest.insert("manifestVersion".to_string(), json!("1.0.0"));
    manifest.insert("name".to_string(), json!(name));
    manifest.insert("description".to_string(), json!(description));
    manifest.insert("version".to_string(), Value::Object(version_object));
    manifest.insert("createdAt".to_string(), json!(timestamp));
    manifest.insert("modifiedAt".to_string(), json!(timestamp));
    manifest.insert("author".to_string(), json!(author));
    manifest.insert("organization".to_string(), json!(organization));
    manifest.insert("archetypes".to_string(), Value::Array(archetypes));
    manifest.insert("sourceFile".to_string(), json!(file));
    let manifest = Value::Object(manifest);
    validate_manifest(&manifest)
        .map_err(|err| CliError::unexpected(format!("manifest validation failed: {err}")))?;
    Ok(manifest)
}

pub(crate) fn pptx_template_inspect(manifest_path: &str) -> CliResult<Value> {
    let text = fs::read_to_string(manifest_path).map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            CliError::file_not_found(format!("file not found: {manifest_path}"))
        } else {
            CliError::unexpected(format!("failed to read manifest file: {err}"))
        }
    })?;
    let manifest: Value = serde_json::from_str(&text)
        .map_err(|err| CliError::unexpected(format!("failed to parse manifest: {err}")))?;
    validate_manifest(&manifest)
        .map_err(|err| CliError::unexpected(format!("manifest validation failed: {err}")))?;
    Ok(manifest_inspect_json(&manifest))
}

pub(crate) fn pptx_template_compile(
    manifest_path: &str,
    spec_path: &str,
    args: &[String],
) -> CliResult<Value> {
    reject_unknown_flags(
        args,
        &["--archetype", "--out", "--image-base-dir"],
        &["--continue-on-error"],
    )?;
    let archetype_arg = parse_string_flag(args, "--archetype")?;
    let out_arg = parse_string_flag(args, "--out")?;
    let missing_required = [
        ("archetype", archetype_arg.as_deref()),
        ("out", out_arg.as_deref()),
    ]
    .into_iter()
    .filter_map(|(name, value)| {
        if value.is_some_and(|value| !value.trim().is_empty()) {
            None
        } else {
            Some(name)
        }
    })
    .collect::<Vec<_>>();
    if !missing_required.is_empty() {
        let quoted = missing_required
            .iter()
            .map(|name| format!("\"{name}\""))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(CliError::invalid_args(format!(
            "required flag(s) {quoted} not set"
        )));
    }
    let archetype_path = archetype_arg.expect("validated archetype flag");
    let out = out_arg.expect("validated out flag");
    if out.trim().is_empty() {
        return Err(CliError::invalid_args("--out is required"));
    }
    if package_type(&archetype_path)? != "pptx" {
        return Err(CliError::unsupported_type(
            "template compile supports PPTX archetype files",
        ));
    }
    let continue_on_error = crate::has_flag(args, "--continue-on-error");
    let _image_base_dir = parse_string_flag(args, "--image-base-dir")?;
    let manifest = read_manifest(manifest_path)?;
    validate_manifest(&manifest)
        .map_err(|err| CliError::unexpected(format!("manifest validation failed: {err}")))?;
    let spec = parse_compile_spec(spec_path)?;
    if spec.slides.is_empty() {
        return Err(CliError::invalid_args(
            "spec must contain at least one slide",
        ));
    }

    let started_at = current_utc_rfc3339();
    let started = Instant::now();
    let seed_slide_count = pptx_slides_list(&archetype_path)?
        .get("slides")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    if seed_slide_count == 0 {
        return Err(CliError::unexpected(
            "template archetype presentation has no slides",
        ));
    }
    let mut working = package_mutation_temp_path(&archetype_path, "pptx-template-compile");
    fs::copy(&archetype_path, &working)
        .map_err(|err| CliError::unexpected(format!("failed to stage archetype: {err}")))?;

    let mut slots_attempted = 0usize;
    let mut slots_succeeded = 0usize;
    let mut errors = Vec::<Value>::new();
    for (slide_index, slide) in spec.slides.iter().enumerate() {
        let archetype = find_archetype(&manifest, &slide.archetype)?;
        let source_slide = archetype
            .get("sourceSlideNumber")
            .and_then(Value::as_i64)
            .unwrap_or(1);
        let current_count = pptx_slides_list(&working)?
            .get("slides")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0);
        let cloned = package_mutation_temp_path(&working, "pptx-template-clone");
        pptx_clone_slide(
            &working,
            &[
                "--slide".to_string(),
                source_slide.to_string(),
                "--insert-after".to_string(),
                current_count.to_string(),
                "--out".to_string(),
                cloned.clone(),
                "--no-validate".to_string(),
            ],
        )?;
        remove_temp_file(&working);
        working = cloned;

        let new_slide = current_count + 1;
        match fill_compiled_slide(
            &working,
            slide_index,
            new_slide,
            &archetype,
            &slide.content,
            &mut slots_attempted,
            &mut slots_succeeded,
        ) {
            Ok(next_working) => {
                if next_working != working {
                    remove_temp_file(&working);
                    working = next_working;
                }
            }
            Err(err) if continue_on_error => {
                errors.push(json!({
                    "slideIndex": slide_index,
                    "message": err.message,
                }));
            }
            Err(err) => {
                remove_temp_file(&working);
                return Err(err);
            }
        }
    }

    for seed in (1..=seed_slide_count).rev() {
        let deleted = package_mutation_temp_path(&working, "pptx-template-delete-seed");
        pptx_slides_delete(
            &working,
            seed as i64,
            &[
                "--out".to_string(),
                deleted.clone(),
                "--no-validate".to_string(),
            ],
        )?;
        remove_temp_file(&working);
        working = deleted;
    }

    validate(&working, true)?;
    if let Some(parent) = Path::new(&out).parent() {
        fs::create_dir_all(parent).map_err(|err| CliError::unexpected(err.to_string()))?;
    }
    fs::copy(&working, &out)
        .map_err(|err| CliError::unexpected(format!("failed to write output file: {err}")))?;
    remove_temp_file(&working);
    let completed_at = current_utc_rfc3339();
    Ok(json!({
        "completedAt": completed_at,
        "duration": format_duration(started.elapsed()),
        "errors": errors,
        "outputPath": out,
        "slideCount": spec.slides.len(),
        "slotsAttempted": slots_attempted,
        "slotsSucceeded": slots_succeeded,
        "startedAt": started_at,
    }))
}

struct CompileSpec {
    slides: Vec<CompileSlide>,
}

struct CompileSlide {
    archetype: String,
    content: BTreeMap<String, SpecValue>,
}

enum SpecValue {
    Scalar(String),
    Object(BTreeMap<String, String>),
}

fn read_manifest(manifest_path: &str) -> CliResult<Value> {
    let text = fs::read_to_string(manifest_path).map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            CliError::file_not_found(format!("file not found: {manifest_path}"))
        } else {
            CliError::unexpected(format!("failed to read manifest file: {err}"))
        }
    })?;
    serde_json::from_str(&text)
        .map_err(|err| CliError::unexpected(format!("failed to parse manifest: {err}")))
}

fn parse_compile_spec(spec_path: &str) -> CliResult<CompileSpec> {
    let text = fs::read_to_string(spec_path).map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            CliError::file_not_found(format!("file not found: {spec_path}"))
        } else {
            CliError::unexpected(format!("failed to read spec file: {err}"))
        }
    })?;
    let mut slides = Vec::<CompileSlide>::new();
    let mut current: Option<CompileSlide> = None;
    let mut in_slides = false;
    let mut in_content = false;
    let mut block_key: Option<String> = None;
    let mut block_lines = Vec::<String>::new();
    let mut object_key: Option<String> = None;

    for raw_line in text.lines() {
        let line = raw_line.trim_end_matches('\r');
        let trimmed = line.trim();
        if block_key.is_some() && (trimmed.is_empty() || leading_spaces(line) >= 8) {
            block_lines.push(strip_indent(line, 8).to_string());
            continue;
        } else if let Some(key) = block_key.take() {
            if let Some(slide) = current.as_mut() {
                slide.content.insert(
                    key,
                    SpecValue::Scalar(block_lines.join("\n").trim_end().to_string()),
                );
            }
            block_lines.clear();
        }
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = leading_spaces(line);
        if indent == 0 && trimmed == "slides:" {
            in_slides = true;
            continue;
        }
        if !in_slides {
            continue;
        }
        if indent == 2 && trimmed.starts_with("- ") {
            if let Some(slide) = current.take() {
                slides.push(slide);
            }
            in_content = false;
            object_key = None;
            let mut slide = CompileSlide {
                archetype: String::new(),
                content: BTreeMap::new(),
            };
            let after_dash = trimmed.trim_start_matches("- ").trim();
            if let Some((key, value)) = parse_yaml_key_value(after_dash)
                && key == "archetype"
            {
                slide.archetype = yaml_scalar(value);
            }
            current = Some(slide);
            continue;
        }
        let Some(slide) = current.as_mut() else {
            continue;
        };
        if indent == 4 && trimmed == "content:" {
            in_content = true;
            object_key = None;
            continue;
        }
        if !in_content {
            if indent == 4
                && let Some((key, value)) = parse_yaml_key_value(trimmed)
                && key == "archetype"
            {
                slide.archetype = yaml_scalar(value);
            }
            continue;
        }
        if indent == 6 {
            object_key = None;
            if let Some((key, value)) = parse_yaml_key_value(trimmed) {
                if value == "|" {
                    block_key = Some(key.to_string());
                    block_lines.clear();
                } else if value.is_empty() {
                    slide
                        .content
                        .insert(key.to_string(), SpecValue::Object(BTreeMap::new()));
                    object_key = Some(key.to_string());
                } else {
                    slide
                        .content
                        .insert(key.to_string(), SpecValue::Scalar(yaml_scalar(value)));
                }
            }
            continue;
        }
        if indent == 8
            && let Some(key) = object_key.as_deref()
            && let Some((nested_key, nested_value)) = parse_yaml_key_value(trimmed)
            && let Some(SpecValue::Object(object)) = slide.content.get_mut(key)
        {
            object.insert(nested_key.to_string(), yaml_scalar(nested_value));
        }
    }
    if let Some(key) = block_key.take()
        && let Some(slide) = current.as_mut()
    {
        slide.content.insert(
            key,
            SpecValue::Scalar(block_lines.join("\n").trim_end().to_string()),
        );
    }
    if let Some(slide) = current.take() {
        slides.push(slide);
    }
    for (index, slide) in slides.iter().enumerate() {
        if slide.archetype.trim().is_empty() {
            return Err(CliError::invalid_args(format!(
                "spec slide {index} is missing archetype"
            )));
        }
    }
    Ok(CompileSpec { slides })
}

fn leading_spaces(line: &str) -> usize {
    line.bytes().take_while(|byte| *byte == b' ').count()
}

fn strip_indent(line: &str, indent: usize) -> &str {
    let mut cursor = 0usize;
    let bytes = line.as_bytes();
    while cursor < indent && cursor < bytes.len() && bytes[cursor] == b' ' {
        cursor += 1;
    }
    &line[cursor..]
}

fn parse_yaml_key_value(line: &str) -> Option<(&str, &str)> {
    let (key, value) = line.split_once(':')?;
    Some((key.trim(), value.trim()))
}

fn yaml_scalar(value: &str) -> String {
    let value = value.trim();
    if value.len() >= 2
        && ((value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\'')))
    {
        value[1..value.len() - 1].replace("\\\"", "\"")
    } else {
        value.to_string()
    }
}

fn find_archetype(manifest: &Value, id: &str) -> CliResult<Value> {
    manifest
        .get("archetypes")
        .and_then(Value::as_array)
        .and_then(|archetypes| {
            archetypes
                .iter()
                .find(|archetype| archetype.get("id").and_then(Value::as_str) == Some(id))
        })
        .cloned()
        .ok_or_else(|| CliError::invalid_args(format!("archetype {id:?} not found in manifest")))
}

fn fill_compiled_slide(
    file: &str,
    slide_index: usize,
    slide_number: usize,
    archetype: &Value,
    content: &BTreeMap<String, SpecValue>,
    slots_attempted: &mut usize,
    slots_succeeded: &mut usize,
) -> CliResult<String> {
    let show = pptx_shapes_show(file, slide_number as u32, true, true)?;
    let part = show
        .get("partUri")
        .and_then(Value::as_str)
        .ok_or_else(|| CliError::unexpected("PPTX shape readback missing partUri"))?
        .trim_start_matches('/')
        .to_string();
    let mut slide_xml = zip_text(file, &part)?;
    let mut changed = false;
    for slot in archetype
        .get("slots")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
    {
        *slots_attempted += 1;
        let slot_id = string_field(&slot, "id");
        let placeholder_key = string_field(&slot, "placeholderKey");
        let required = slot
            .get("required")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let Some(value) = content
            .get(&slot_id)
            .or_else(|| content.get(&placeholder_key))
        else {
            if required {
                return Err(CliError::invalid_args(format!(
                    "slide {slide_index}: required slot {slot_id:?} is missing"
                )));
            }
            continue;
        };
        let kind = string_field(&slot, "kind");
        if !matches!(kind.as_str(), "text" | "richText" | "bullets") {
            return Err(CliError::invalid_args(format!(
                "slide {slide_index}: slot {slot_id:?} kind {kind:?} is not implemented in Rust template compile"
            )));
        }
        let text = compile_slot_text(value, &kind, &slot_id)?;
        let target = if placeholder_key.is_empty() {
            slot_id.as_str()
        } else {
            placeholder_key.as_str()
        };
        let shape_id = resolve_slot_shape_id(&show, target).ok_or_else(|| {
            CliError::unexpected(format!(
                "slide {slide_index}: target shape {target:?} not found"
            ))
        })?;
        slide_xml = set_shape_text(&slide_xml, shape_id, &text)?;
        *slots_succeeded += 1;
        changed = true;
    }
    if !changed {
        return Ok(file.to_string());
    }
    let mut overrides = BTreeMap::new();
    overrides.insert(part, slide_xml);
    let out = package_mutation_temp_path(file, "pptx-template-fill");
    copy_zip_with_part_overrides(file, &out, &overrides)?;
    Ok(out)
}

fn compile_slot_text(value: &SpecValue, kind: &str, slot_id: &str) -> CliResult<String> {
    let SpecValue::Scalar(text) = value else {
        return Err(CliError::invalid_args(format!(
            "slot {slot_id:?} requires scalar text content"
        )));
    };
    if kind != "bullets" {
        return Ok(text.clone());
    }
    Ok(text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| {
            line.strip_prefix("- ")
                .or_else(|| line.strip_prefix("* "))
                .or_else(|| line.strip_prefix("\u{2022} "))
                .unwrap_or(line)
                .trim()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n"))
}

fn resolve_slot_shape_id(show: &Value, target: &str) -> Option<u32> {
    show.get("shapes")
        .and_then(Value::as_array)?
        .iter()
        .find(|shape| {
            shape.get("primarySelector").and_then(Value::as_str) == Some(target)
                || shape.get("targetKind").and_then(Value::as_str) == Some(target)
                || shape
                    .get("placeholder")
                    .and_then(|placeholder| placeholder.get("key"))
                    .and_then(Value::as_str)
                    == Some(target)
                || shape
                    .get("selectors")
                    .and_then(Value::as_array)
                    .is_some_and(|selectors| {
                        selectors
                            .iter()
                            .any(|selector| selector.as_str() == Some(target))
                    })
        })
        .and_then(|shape| shape.get("shapeId").and_then(Value::as_u64))
        .map(|id| id as u32)
}

fn set_shape_text(xml: &str, shape_id: u32, text: &str) -> CliResult<String> {
    let spans = element_spans(xml, "sp", 0, xml.len());
    for span in spans {
        let fragment = &xml[span.0..span.1];
        if !shape_fragment_has_id(fragment, shape_id) {
            continue;
        }
        let tx_body = first_element_span(xml, "txBody", span.0, span.1)
            .ok_or_else(|| CliError::unexpected(format!("shape {shape_id} has no text body")))?;
        let replacement = render_text_body(text);
        return Ok(replace_span(xml, tx_body.0, tx_body.1, &replacement));
    }
    Err(CliError::unexpected(format!(
        "shape {shape_id} not found in slide XML"
    )))
}

fn shape_fragment_has_id(fragment: &str, shape_id: u32) -> bool {
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                if local_name(e.name().as_ref()) == "cNvPr"
                    && attr(&e, "id").and_then(|value| value.parse::<u32>().ok()) == Some(shape_id)
                {
                    return true;
                }
            }
            Ok(Event::Eof) => return false,
            Err(_) => return false,
            _ => {}
        }
    }
}

fn render_text_body(text: &str) -> String {
    format!(
        "<p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>{}</a:t></a:r></a:p></p:txBody>",
        xml_escape(text)
    )
}

fn first_element_span(
    xml: &str,
    wanted: &str,
    range_start: usize,
    range_end: usize,
) -> Option<(usize, usize)> {
    element_spans(xml, wanted, range_start, range_end)
        .into_iter()
        .next()
}

fn element_spans(
    xml: &str,
    wanted: &str,
    range_start: usize,
    range_end: usize,
) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut cursor = range_start;
    while cursor < range_end {
        let Some(relative_start) = xml[cursor..range_end].find('<') else {
            break;
        };
        let tag_start = cursor + relative_start;
        let Some(relative_end) = xml[tag_start..range_end].find('>') else {
            break;
        };
        let tag_end = tag_start + relative_end;
        let token = xml[tag_start + 1..tag_end].trim_start();
        if token.starts_with('/') || token.starts_with('?') || token.starts_with('!') {
            cursor = tag_end + 1;
            continue;
        }
        let Some(name) = xml_token_name(token) else {
            cursor = tag_end + 1;
            continue;
        };
        if local_name(name.as_bytes()) == wanted {
            if token.trim_end().ends_with('/') {
                spans.push((tag_start, tag_end + 1));
                cursor = tag_end + 1;
                continue;
            }
            if let Some(end) = find_matching_element_end(xml, wanted, tag_end + 1, range_end) {
                spans.push((tag_start, end));
                cursor = end;
                continue;
            }
        }
        cursor = tag_end + 1;
    }
    spans
}

fn find_matching_element_end(
    xml: &str,
    wanted: &str,
    range_start: usize,
    range_end: usize,
) -> Option<usize> {
    let mut depth = 1usize;
    let mut cursor = range_start;
    while cursor < range_end {
        let relative_start = xml[cursor..range_end].find('<')?;
        let tag_start = cursor + relative_start;
        let relative_end = xml[tag_start..range_end].find('>')?;
        let tag_end = tag_start + relative_end;
        let token = xml[tag_start + 1..tag_end].trim_start();
        if token.starts_with('?') || token.starts_with('!') {
            cursor = tag_end + 1;
            continue;
        }
        if let Some(name) = xml_token_name(token)
            && local_name(name.as_bytes()) == wanted
        {
            if token.starts_with('/') {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(tag_end + 1);
                }
            } else if !token.trim_end().ends_with('/') {
                depth += 1;
            }
        }
        cursor = tag_end + 1;
    }
    None
}

fn replace_span(xml: &str, start: usize, end: usize, replacement: &str) -> String {
    let mut out = String::with_capacity(xml.len() - (end - start) + replacement.len());
    out.push_str(&xml[..start]);
    out.push_str(replacement);
    out.push_str(&xml[end..]);
    out
}

fn format_duration(duration: std::time::Duration) -> String {
    if duration.as_secs() > 0 {
        format!("{:.3}s", duration.as_secs_f64())
    } else {
        format!("{:.4}ms", duration.as_secs_f64() * 1000.0)
    }
}

fn remove_temp_file(path: &str) {
    let _ = fs::remove_file(path);
}

fn capture_archetype(file: &str, slide: u32, slide_meta: &Value) -> CliResult<Value> {
    let shapes = pptx_shapes_show(file, slide, true, true)?;
    let shape_items = shapes
        .get("shapes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut slots = Vec::<Value>::new();
    let mut static_shapes = Vec::<Value>::new();
    let mut seen_slot_ids = std::collections::BTreeMap::<String, usize>::new();
    for shape in shape_items {
        if let Some(slot) = slot_from_shape(&shape, &mut seen_slot_ids) {
            slots.push(slot);
        } else {
            static_shapes.push(static_shape_from_shape(&shape));
        }
    }
    if slots.is_empty() {
        return Err(CliError::unexpected(format!(
            "template capture failed: failed to capture slide {slide}: slide has no fillable slots or placeholders"
        )));
    }
    let mut archetype = Map::new();
    archetype.insert("id".to_string(), json!(format!("archetype-{slide}")));
    archetype.insert("name".to_string(), json!(format!("Slide {slide}")));
    archetype.insert("description".to_string(), json!(""));
    archetype.insert("slots".to_string(), Value::Array(slots));
    archetype.insert("staticShapes".to_string(), Value::Array(static_shapes));
    archetype.insert(
        "layoutName".to_string(),
        json!(
            slide_meta
                .get("layout")
                .and_then(Value::as_str)
                .or_else(|| shapes.get("layoutName").and_then(Value::as_str))
                .unwrap_or_default()
        ),
    );
    archetype.insert("masterName".to_string(), json!(""));
    archetype.insert("sourceSlideNumber".to_string(), json!(slide));
    Ok(Value::Object(archetype))
}

fn slot_from_shape(
    shape: &Value,
    seen_slot_ids: &mut std::collections::BTreeMap<String, usize>,
) -> Option<Value> {
    let target_kind = shape
        .get("targetKind")
        .and_then(Value::as_str)
        .unwrap_or("");
    let shape_type = shape.get("shapeType").and_then(Value::as_str).unwrap_or("");
    let text_capable = shape
        .get("textCapable")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let table_info = shape.get("tableInfo");
    let kind = if table_info.is_some() || target_kind == "table" {
        "table"
    } else if target_kind == "picture" || shape_type == "pic" {
        "image"
    } else if target_kind == "body" {
        "bullets"
    } else if text_capable {
        "text"
    } else {
        return None;
    };
    let placeholder = shape.get("placeholder");
    let placeholder_role = placeholder
        .and_then(|placeholder| placeholder.get("role"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let base_id = placeholder
        .and_then(|placeholder| placeholder.get("key"))
        .and_then(Value::as_str)
        .or_else(|| shape.get("primarySelector").and_then(Value::as_str))
        .map(slot_id_from_selector)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| {
            format!(
                "shape-{}",
                shape.get("shapeId").and_then(Value::as_i64).unwrap_or(0)
            )
        });
    let count = seen_slot_ids.entry(base_id.clone()).or_default();
    *count += 1;
    let slot_id = if *count == 1 {
        base_id
    } else {
        format!("{base_id}-{}", *count)
    };
    let mut slot = Map::new();
    slot.insert("id".to_string(), json!(slot_id));
    slot.insert(
        "name".to_string(),
        json!(slot_name(shape, placeholder_role)),
    );
    slot.insert("kind".to_string(), json!(kind));
    slot.insert(
        "required".to_string(),
        json!(matches!(placeholder_role, "title" | "body")),
    );
    if let Some(key) = placeholder
        .and_then(|placeholder| placeholder.get("key"))
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
    {
        slot.insert("placeholderKey".to_string(), json!(key));
    }
    if !placeholder_role.is_empty() {
        slot.insert("placeholderRole".to_string(), json!(placeholder_role));
    }
    if let Some(bounds) = shape.get("bounds") {
        slot.insert("bounds".to_string(), bounds.clone());
    }
    if kind == "table" {
        if let Some(rows) = table_info
            .and_then(|info| info.get("rows"))
            .and_then(Value::as_array)
        {
            slot.insert("tableRows".to_string(), json!(rows.len()));
        }
        if let Some(cols) = table_info
            .and_then(|info| info.get("cols"))
            .or_else(|| table_info.and_then(|info| info.get("columns")))
            .and_then(Value::as_array)
        {
            slot.insert("tableCols".to_string(), json!(cols.len()));
        }
        if let Some(primary) = shape.get("primarySelector").and_then(Value::as_str) {
            slot.insert("tableId".to_string(), json!(primary));
        }
    }
    if kind == "image"
        && let Some(bounds) = shape.get("bounds")
        && let (Some(cx), Some(cy)) = (
            bounds.get("cx").and_then(Value::as_f64),
            bounds.get("cy").and_then(Value::as_f64),
        )
        && cy > 0.0
    {
        slot.insert("aspectRatio".to_string(), json!(cx / cy));
    }
    Some(Value::Object(slot))
}

fn static_shape_from_shape(shape: &Value) -> Value {
    let mut out = Map::new();
    out.insert(
        "id".to_string(),
        json!(format!(
            "shape-{}",
            shape.get("shapeId").and_then(Value::as_i64).unwrap_or(0)
        )),
    );
    out.insert(
        "name".to_string(),
        json!(shape.get("shapeName").and_then(Value::as_str).unwrap_or("")),
    );
    out.insert(
        "type".to_string(),
        json!(
            shape
                .get("shapeType")
                .and_then(Value::as_str)
                .unwrap_or("shape")
        ),
    );
    if let Some(bounds) = shape.get("bounds") {
        out.insert("bounds".to_string(), bounds.clone());
    }
    Value::Object(out)
}

fn slot_name(shape: &Value, placeholder_role: &str) -> String {
    if !placeholder_role.is_empty() {
        return placeholder_role
            .split(['-', '_', ':'])
            .filter(|part| !part.is_empty())
            .map(|part| {
                let mut chars = part.chars();
                match chars.next() {
                    Some(first) => {
                        format!("{}{}", first.to_ascii_uppercase(), chars.as_str())
                    }
                    None => String::new(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
    }
    shape
        .get("shapeName")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .unwrap_or("Slot")
        .to_string()
}

fn slot_id_from_selector(selector: &str) -> String {
    selector
        .trim_start_matches('@')
        .replace(':', "-")
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

fn parse_version(version: &str) -> CliResult<(i64, i64, i64)> {
    let parts = version.split('.').collect::<Vec<_>>();
    if parts.len() != 3 {
        return Err(CliError::invalid_args(
            "--version must be major.minor.patch",
        ));
    }
    let major = parse_version_part(parts[0], "major")?;
    let minor = parse_version_part(parts[1], "minor")?;
    let patch = parse_version_part(parts[2], "patch")?;
    Ok((major, minor, patch))
}

fn parse_version_part(part: &str, label: &str) -> CliResult<i64> {
    let value = part.parse::<i64>().map_err(|_| {
        CliError::invalid_args(format!(
            "--version {label} component must be a non-negative integer"
        ))
    })?;
    if value < 0 {
        return Err(CliError::invalid_args(format!(
            "--version {label} component must be a non-negative integer"
        )));
    }
    Ok(value)
}

fn parse_capture_slides(slides: Option<&str>, file: &str) -> CliResult<Vec<u32>> {
    let all_slides = pptx_all_slides(file);
    let slide_count = all_slides.len() as u32;
    let Some(slides) = slides.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(all_slides);
    };
    let mut out = Vec::new();
    for piece in slides.split(',') {
        let slide = piece.trim().parse::<u32>().map_err(|_| {
            CliError::invalid_args(format!("invalid slide number in --slides: {piece:?}"))
        })?;
        if slide == 0 || slide > slide_count {
            return Err(CliError::invalid_args(format!(
                "slide {slide} is out of range (presentation has {slide_count} slides)"
            )));
        }
        if !out.contains(&slide) {
            out.push(slide);
        }
    }
    Ok(out)
}

fn manifest_inspect_json(manifest: &Value) -> Value {
    let mut output = Map::new();
    output.insert("name".to_string(), json!(string_field(manifest, "name")));
    output.insert(
        "description".to_string(),
        json!(string_field(manifest, "description")),
    );
    output.insert("version".to_string(), json!(version_string(manifest)));
    output.insert(
        "author".to_string(),
        json!(string_field(manifest, "author")),
    );
    output.insert(
        "organization".to_string(),
        json!(string_field(manifest, "organization")),
    );
    output.insert(
        "createdAt".to_string(),
        manifest.get("createdAt").cloned().unwrap_or(Value::Null),
    );
    output.insert(
        "modifiedAt".to_string(),
        manifest.get("modifiedAt").cloned().unwrap_or(Value::Null),
    );
    output.insert(
        "archetypes".to_string(),
        Value::Array(
            manifest
                .get("archetypes")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
                .iter()
                .map(archetype_inspect_json)
                .collect(),
        ),
    );
    Value::Object(output)
}

fn archetype_inspect_json(archetype: &Value) -> Value {
    let mut output = Map::new();
    output.insert("id".to_string(), json!(string_field(archetype, "id")));
    output.insert("name".to_string(), json!(string_field(archetype, "name")));
    output.insert(
        "description".to_string(),
        json!(string_field(archetype, "description")),
    );
    output.insert(
        "layoutName".to_string(),
        json!(string_field(archetype, "layoutName")),
    );
    output.insert(
        "masterName".to_string(),
        json!(string_field(archetype, "masterName")),
    );
    output.insert(
        "slots".to_string(),
        Value::Array(
            archetype
                .get("slots")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
                .iter()
                .map(slot_inspect_json)
                .collect(),
        ),
    );
    output.insert(
        "staticShapes".to_string(),
        Value::Array(
            archetype
                .get("staticShapes")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
                .iter()
                .map(static_shape_inspect_json)
                .collect(),
        ),
    );
    Value::Object(output)
}

fn slot_inspect_json(slot: &Value) -> Value {
    let mut output = Map::new();
    let kind = string_field(slot, "kind");
    output.insert("id".to_string(), json!(string_field(slot, "id")));
    output.insert("name".to_string(), json!(string_field(slot, "name")));
    output.insert("kind".to_string(), json!(kind));
    output.insert(
        "required".to_string(),
        json!(
            slot.get("required")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        ),
    );
    if let Some(role) = non_empty_string_field(slot, "placeholderRole") {
        output.insert("placeholderRole".to_string(), json!(role));
    }
    if kind == "table" {
        if let Some(rows) = slot.get("tableRows").and_then(Value::as_i64) {
            output.insert("tableRows".to_string(), json!(rows));
        }
        if let Some(cols) = slot.get("tableCols").and_then(Value::as_i64) {
            output.insert("tableCols".to_string(), json!(cols));
        }
    }
    Value::Object(output)
}

fn static_shape_inspect_json(shape: &Value) -> Value {
    let mut output = Map::new();
    output.insert("id".to_string(), json!(string_field(shape, "id")));
    output.insert("name".to_string(), json!(string_field(shape, "name")));
    output.insert("type".to_string(), json!(string_field(shape, "type")));
    Value::Object(output)
}

fn validate_manifest(manifest: &Value) -> Result<(), String> {
    if !manifest.is_object() {
        return Err("manifest is nil".to_string());
    }
    if string_field(manifest, "name").is_empty() {
        return Err("manifest must have a non-empty name".to_string());
    }
    if string_field(manifest, "manifestVersion").is_empty() {
        return Err("manifest must have a manifestVersion".to_string());
    }
    let version = manifest
        .get("version")
        .ok_or_else(|| "manifest must have version information".to_string())?;
    validate_version(version).map_err(|err| format!("invalid version: {err}"))?;
    let archetypes = manifest
        .get("archetypes")
        .and_then(Value::as_array)
        .ok_or_else(|| "manifest must have at least one archetype".to_string())?;
    if archetypes.is_empty() {
        return Err("manifest must have at least one archetype".to_string());
    }
    let mut seen = std::collections::BTreeSet::new();
    for (index, archetype) in archetypes.iter().enumerate() {
        let id = string_field(archetype, "id");
        if id.is_empty() {
            return Err(format!("archetype at index {index} has empty ID"));
        }
        if !seen.insert(id.clone()) {
            return Err(format!("duplicate archetype ID: {id}"));
        }
        validate_archetype(archetype).map_err(|err| format!("archetype {id} is invalid: {err}"))?;
    }
    Ok(())
}

fn validate_version(version: &Value) -> Result<(), String> {
    if !version.is_object() {
        return Err("version is nil".to_string());
    }
    let major = version.get("major").and_then(Value::as_i64).unwrap_or(0);
    let minor = version.get("minor").and_then(Value::as_i64).unwrap_or(0);
    let patch = version.get("patch").and_then(Value::as_i64).unwrap_or(0);
    if major < 0 || minor < 0 || patch < 0 {
        return Err(format!(
            "version numbers must be non-negative, got {major}.{minor}.{patch}"
        ));
    }
    if non_empty_string_field(version, "createdAt").is_none() {
        return Err("version must have a createdAt timestamp".to_string());
    }
    Ok(())
}

fn validate_archetype(archetype: &Value) -> Result<(), String> {
    let id = string_field(archetype, "id");
    if id.is_empty() {
        return Err("archetype must have an ID".to_string());
    }
    if string_field(archetype, "name").is_empty() {
        return Err(format!("archetype {id} must have a name"));
    }
    let slots = archetype
        .get("slots")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("archetype {id} must have at least one slot"))?;
    if slots.is_empty() {
        return Err(format!("archetype {id} must have at least one slot"));
    }
    let mut seen = std::collections::BTreeSet::new();
    for (index, slot) in slots.iter().enumerate() {
        let slot_id = string_field(slot, "id");
        if slot_id.is_empty() {
            return Err(format!(
                "archetype {id}: slot at index {index} has empty ID"
            ));
        }
        if !seen.insert(slot_id.clone()) {
            return Err(format!("archetype {id}: duplicate slot ID {slot_id}"));
        }
        validate_slot(slot)
            .map_err(|err| format!("archetype {id}, slot {slot_id} is invalid: {err}"))?;
    }
    Ok(())
}

fn validate_slot(slot: &Value) -> Result<(), String> {
    let id = string_field(slot, "id");
    if id.is_empty() {
        return Err("slot must have an ID".to_string());
    }
    if string_field(slot, "name").is_empty() {
        return Err(format!("slot {id} must have a name"));
    }
    let kind = string_field(slot, "kind");
    if !matches!(
        kind.as_str(),
        "text" | "richText" | "bullets" | "image" | "table" | "notes"
    ) {
        return Err(format!("slot {id}: invalid kind {kind:?}"));
    }
    if let Some(bounds) = slot.get("bounds").and_then(Value::as_object) {
        let cx = bounds.get("cx").and_then(Value::as_i64).unwrap_or(0);
        let cy = bounds.get("cy").and_then(Value::as_i64).unwrap_or(0);
        if cx <= 0 || cy <= 0 {
            return Err(format!(
                "slot {id}: bounds must have positive width and height"
            ));
        }
    }
    if kind == "table" {
        if let Some(rows) = slot.get("tableRows").and_then(Value::as_i64)
            && rows <= 0
        {
            return Err(format!("slot {id}: tableRows must be positive"));
        }
        if let Some(cols) = slot.get("tableCols").and_then(Value::as_i64)
            && cols <= 0
        {
            return Err(format!("slot {id}: tableCols must be positive"));
        }
    }
    if let Some(ratio) = slot.get("aspectRatio").and_then(Value::as_f64)
        && ratio <= 0.0
    {
        return Err(format!("slot {id}: aspectRatio must be positive"));
    }
    Ok(())
}

fn version_string(manifest: &Value) -> String {
    let version = manifest.get("version").unwrap_or(&Value::Null);
    format!(
        "{}.{}.{}",
        version.get("major").and_then(Value::as_i64).unwrap_or(0),
        version.get("minor").and_then(Value::as_i64).unwrap_or(0),
        version.get("patch").and_then(Value::as_i64).unwrap_or(0)
    )
}

fn string_field(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn non_empty_string_field(value: &Value, key: &str) -> Option<String> {
    let value = string_field(value, key);
    if value.is_empty() { None } else { Some(value) }
}
