use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use crate::{
    CliError, CliResult, XmlNamedRange, attr, attr_exact, copy_zip_with_part_overrides,
    current_utc_rfc3339, has_flag, local_name, needs_xml_space_preserve,
    package_mutation_temp_path, package_type, parse_string_flag, parse_string_flags,
    relationship_entries, relationships_part_for, replace_xml_span, resolve_relationship_target,
    xml_direct_child_ranges, xml_escape, xml_fragment_bounds, xml_tag_prefix, zip_text,
};

const MANIFEST_VERSION: &str = "1.0.0";
const PRESENTATION_PART: &str = "ppt/presentation.xml";
const REL_TYPE_SLIDE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide";
const REL_TYPE_NOTES_SLIDE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesSlide";

#[derive(Clone)]
struct ExportOptions {
    slides: Vec<u32>,
    include_notes: bool,
    source_language: Option<String>,
    target_language: Option<String>,
}

#[derive(Clone, Debug)]
struct SlideRef {
    slide_number: i64,
    part: String,
    notes_part: Option<String>,
}

#[derive(Clone, Debug, Default)]
struct ShapeInfo {
    id: Option<i64>,
    name: String,
    placeholder_type: String,
    placeholder_idx: Option<String>,
    key: String,
}

#[derive(Clone, Debug)]
struct EntryData {
    id: String,
    entry_type: String,
    source_text: String,
    target_text: String,
    slide_id: i64,
    slide_number: i64,
    placeholder_key: String,
    shape_id: Option<i64>,
    shape_name: String,
    paragraph_index: i64,
    run_index: i64,
    segment_type: String,
    bullet_info: Option<Map<String, Value>>,
    run_format: Option<Map<String, Value>>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum StaleMode {
    Skip,
    Warn,
    Error,
}

pub(crate) fn pptx_translate_export(file: &str, args: &[String]) -> CliResult<Value> {
    ensure_pptx_package(file)?;
    let options = parse_export_options(args)?;
    let slides = pptx_slide_refs(file)?;
    let selected = select_slides(&slides, &options.slides);
    let entries = export_entries(file, &selected, options.include_notes)?;

    let mut metadata = Map::new();
    metadata.insert("version".to_string(), json!(MANIFEST_VERSION));
    metadata.insert("exportedAt".to_string(), json!(current_utc_rfc3339()));
    if let Some(source) = options.source_language.filter(|value| !value.is_empty()) {
        metadata.insert("sourceLanguage".to_string(), json!(source));
    }
    if let Some(target) = options.target_language.filter(|value| !value.is_empty()) {
        metadata.insert("targetLanguage".to_string(), json!(target));
    }
    metadata.insert("deckName".to_string(), json!(file));
    metadata.insert("slideCount".to_string(), json!(slides.len()));
    if !entries.is_empty() {
        metadata.insert("entryCount".to_string(), json!(entries.len()));
    }

    Ok(json!({
        "metadata": Value::Object(metadata),
        "entries": entries.into_iter().map(entry_to_json).collect::<Vec<_>>(),
    }))
}

pub(crate) fn pptx_translate_apply(
    file: &str,
    manifest_path: &str,
    args: &[String],
) -> CliResult<Value> {
    let mode = parse_stale_mode(args)?;
    let output = parse_string_flag(args, "--output")?;
    let manifest = read_manifest_json(manifest_path)?;
    ensure_pptx_package(file)?;
    let entries = validate_manifest_entries(&manifest)?;

    let slides = pptx_slide_refs(file)?;
    let current_entries = export_entries(file, &slides, false)?;
    let current_text = current_entries
        .into_iter()
        .map(|entry| (entry.id, entry.source_text))
        .collect::<BTreeMap<_, _>>();

    let mut overrides = BTreeMap::<String, String>::new();
    let mut processed = 0_i64;
    let mut applied = 0_i64;
    let mut skipped = 0_i64;
    let mut warnings = Vec::<String>::new();

    for entry in entries {
        processed += 1;
        if entry.slide_id < 0 || entry.slide_id as usize >= slides.len() {
            warn_apply(
                &mut warnings,
                format!(
                    "entry {}: slide ID {} out of range",
                    entry.id, entry.slide_id
                ),
            );
            skipped += 1;
            continue;
        }

        if !entry.source_text.is_empty() {
            match current_text.get(&entry.id) {
                None => {
                    let msg = format!(
                        "entry {}: text location not found - entry is stale",
                        entry.id
                    );
                    warn_apply(&mut warnings, msg.clone());
                    if mode == StaleMode::Error {
                        return Err(CliError::unexpected(format!(
                            "translation apply failed: {msg}"
                        )));
                    }
                    skipped += 1;
                    continue;
                }
                Some(current) if current != &entry.source_text => {
                    let msg = format!(
                        "entry {}: source text mismatch (expected {:?}, found {:?}) - entry is stale",
                        entry.id, entry.source_text, current
                    );
                    warn_apply(&mut warnings, msg.clone());
                    if mode == StaleMode::Error {
                        return Err(CliError::unexpected(format!(
                            "translation apply failed: {msg}"
                        )));
                    }
                    if mode == StaleMode::Skip {
                        skipped += 1;
                        continue;
                    }
                }
                _ => {}
            }
        }

        if entry.target_text.is_empty() {
            skipped += 1;
            continue;
        }

        let slide = &slides[entry.slide_id as usize];
        let apply_result = if entry.placeholder_key == "notes" {
            apply_entry_to_notes(file, slide, &entry, &mut overrides)
        } else {
            apply_entry_to_slide(file, slide, &entry, &mut overrides)
        };

        match apply_result {
            Ok(()) => applied += 1,
            Err(err) => {
                let msg = format!(
                    "entry {}: failed to apply translation - {}",
                    entry.id, err.message
                );
                warn_apply(&mut warnings, msg);
                skipped += 1;
            }
        }
    }

    let output_path = output.as_deref().unwrap_or(file);
    let write_path = if output_path == file {
        package_mutation_temp_path(file, "pptx-translate")
    } else {
        output_path.to_string()
    };
    copy_zip_with_part_overrides(file, &write_path, &overrides)?;
    if output_path == file {
        fs::rename(&write_path, file).map_err(|err| CliError::unexpected(err.to_string()))?;
    }

    let mut result = Map::new();
    result.insert("entriesProcessed".to_string(), json!(processed));
    result.insert("entriesApplied".to_string(), json!(applied));
    result.insert("entriesSkipped".to_string(), json!(skipped));
    if !warnings.is_empty() {
        result.insert("warnings".to_string(), json!(warnings));
    }
    Ok(Value::Object(result))
}

fn parse_export_options(args: &[String]) -> CliResult<ExportOptions> {
    if let Some(format) = parse_string_flag(args, "--format")?
        && format != "json"
    {
        return Err(CliError::invalid_args(format!(
            "invalid format: {format} (expected 'text' or 'json')"
        )));
    }
    let slides = parse_string_flags(args, "--slide")?
        .into_iter()
        .map(|value| {
            value
                .parse::<u32>()
                .map_err(|_| CliError::invalid_args("--slide must be an integer"))
        })
        .collect::<CliResult<Vec<_>>>()?;
    Ok(ExportOptions {
        slides,
        include_notes: has_flag(args, "--include-notes"),
        source_language: parse_string_flag(args, "--source-lang")?,
        target_language: parse_string_flag(args, "--target-lang")?,
    })
}

fn parse_stale_mode(args: &[String]) -> CliResult<StaleMode> {
    match parse_string_flag(args, "--stale")?
        .as_deref()
        .unwrap_or("skip")
    {
        "skip" => Ok(StaleMode::Skip),
        "warn" => Ok(StaleMode::Warn),
        "error" => Ok(StaleMode::Error),
        other => Err(CliError::invalid_args(format!(
            "invalid --stale mode: {other:?} (must be 'skip', 'warn', or 'error')"
        ))),
    }
}

fn read_manifest_json(path: &str) -> CliResult<Value> {
    let text = fs::read_to_string(path).map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            CliError::file_not_found(format!("file not found: {path}"))
        } else {
            CliError::unexpected(err.to_string())
        }
    })?;
    serde_json::from_str(&text)
        .map_err(|err| CliError::unexpected(format!("failed to parse manifest JSON: {err}")))
}

fn validate_manifest_entries(manifest: &Value) -> CliResult<Vec<EntryData>> {
    let mut issues = Vec::<String>::new();
    if manifest.get("metadata").is_none() || manifest.get("metadata") == Some(&Value::Null) {
        issues.push("[missing-metadata] entry 0: metadata is nil".to_string());
    }

    let entries_value = manifest.get("entries");
    let entries_array = entries_value.and_then(Value::as_array).ok_or_else(|| {
        CliError::unexpected(
            "translation apply failed: manifest validation failed: [missing-entries] entries is nil",
        )
    })?;

    let mut seen = BTreeSet::<String>::new();
    let mut entries = Vec::new();
    for (index, value) in entries_array.iter().enumerate() {
        let id = string_field(value, "id");
        if id.is_empty() || !validate_entry_id(&id) {
            let label = if id.is_empty() {
                format!("entry {index}")
            } else {
                format!("entry {id}")
            };
            issues.push(format!(
                "[invalid-id] {label}: entry ID does not match expected format"
            ));
        } else if !seen.insert(id.clone()) {
            issues.push(format!("[duplicate-id] entry {id}: duplicate entry ID"));
        }

        if string_field(value, "type").is_empty() {
            let label = if id.is_empty() {
                index.to_string()
            } else {
                id.clone()
            };
            issues.push(format!("[missing-type] entry {label}: type is required"));
        }

        let slide_id = int_field(value, "slideId").unwrap_or(0);
        if slide_id < 0 {
            issues.push(format!(
                "[invalid-slide-id] entry {id}: slide ID cannot be negative"
            ));
        }
        let slide_number = int_field(value, "slideNumber").unwrap_or(0);
        if slide_number <= 0 {
            issues.push(format!(
                "[invalid-slide-number] entry {id}: slide number must be positive"
            ));
        }
        let paragraph_index = int_field(value, "paragraphIndex").unwrap_or(0);
        if paragraph_index < 0 {
            issues.push(format!(
                "[invalid-paragraph-index] entry {id}: paragraph index cannot be negative"
            ));
        }
        let run_index = int_field(value, "runIndex").unwrap_or(0);
        if run_index < 0 {
            issues.push(format!(
                "[invalid-run-index] entry {id}: run index cannot be negative"
            ));
        }

        entries.push(EntryData {
            id,
            entry_type: string_field(value, "type"),
            source_text: string_field(value, "sourceText"),
            target_text: string_field(value, "targetText"),
            slide_id,
            slide_number,
            placeholder_key: string_field(value, "placeholderKey"),
            shape_id: int_field(value, "shapeId"),
            shape_name: string_field(value, "shapeName"),
            paragraph_index,
            run_index,
            segment_type: string_field(value, "segmentType"),
            bullet_info: None,
            run_format: None,
        });
    }

    if !issues.is_empty() {
        return Err(CliError::unexpected(format!(
            "translation apply failed: manifest validation failed: {}",
            issues.join("; ")
        )));
    }
    Ok(entries)
}

fn validate_entry_id(id: &str) -> bool {
    let Some(rest) = id.strip_prefix("slide:") else {
        return false;
    };
    let parts = rest.split('_').collect::<Vec<_>>();
    if parts.len() != 4 || parts[1].is_empty() {
        return false;
    }
    parts[0].parse::<i64>().is_ok()
        && parts[2]
            .strip_prefix('p')
            .is_some_and(|value| value.parse::<i64>().is_ok())
        && parts[3]
            .strip_prefix('r')
            .is_some_and(|value| value.parse::<i64>().is_ok())
}

fn string_field(value: &Value, name: &str) -> String {
    value
        .get(name)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn int_field(value: &Value, name: &str) -> Option<i64> {
    value.get(name).and_then(Value::as_i64)
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

fn pptx_slide_refs(file: &str) -> CliResult<Vec<SlideRef>> {
    let pres_xml = zip_text(file, PRESENTATION_PART)?;
    let pres_rels = relationship_entries(file, &relationships_part_for(PRESENTATION_PART))?;
    let rels_by_id = pres_rels
        .into_iter()
        .map(|rel| (rel.id.clone(), rel))
        .collect::<BTreeMap<_, _>>();

    let mut reader = Reader::from_str(&pres_xml);
    reader.config_mut().trim_text(false);
    let mut slides = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "sldId" =>
            {
                let rel_id = attr_exact(&e, "r:id").unwrap_or_default();
                let slide_rel = rels_by_id.get(&rel_id).or_else(|| {
                    rels_by_id
                        .values()
                        .find(|rel| rel.rel_type == REL_TYPE_SLIDE && rel.id == rel_id)
                });
                if let Some(rel) = slide_rel {
                    let part = resolve_relationship_target("/ppt/presentation.xml", &rel.target)
                        .trim_start_matches('/')
                        .to_string();
                    let notes_part = slide_notes_part(file, &part);
                    slides.push(SlideRef {
                        slide_number: slides.len() as i64 + 1,
                        part,
                        notes_part,
                    });
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(slides)
}

fn slide_notes_part(file: &str, slide_part: &str) -> Option<String> {
    relationship_entries(file, &relationships_part_for(slide_part))
        .unwrap_or_default()
        .into_iter()
        .find(|rel| rel.rel_type == REL_TYPE_NOTES_SLIDE || rel.rel_type.ends_with("/notesSlide"))
        .map(|rel| {
            resolve_relationship_target(&format!("/{slide_part}"), &rel.target)
                .trim_start_matches('/')
                .to_string()
        })
}

fn select_slides(slides: &[SlideRef], requested: &[u32]) -> Vec<SlideRef> {
    if requested.is_empty() {
        return slides.to_vec();
    }
    requested
        .iter()
        .filter_map(|number| {
            number
                .checked_sub(1)
                .and_then(|index| slides.get(index as usize))
                .cloned()
        })
        .collect()
}

fn export_entries(
    file: &str,
    slides: &[SlideRef],
    include_notes: bool,
) -> CliResult<Vec<EntryData>> {
    let mut entries = Vec::new();
    for slide in slides {
        let slide_xml = zip_text(file, &slide.part)?;
        entries.extend(entries_from_slide_xml(&slide_xml, slide)?);
        if include_notes
            && let Some(notes_part) = &slide.notes_part
            && let Ok(notes_xml) = zip_text(file, notes_part)
        {
            entries.extend(entries_from_notes_xml(&notes_xml, slide)?);
        }
    }
    Ok(entries)
}

fn entries_from_slide_xml(xml: &str, slide: &SlideRef) -> CliResult<Vec<EntryData>> {
    let mut entries = Vec::new();
    for shape in element_ranges_by_local(xml, "sp")? {
        let fragment = &xml[shape.start..shape.end];
        let info = parse_shape_info(fragment)?;
        entries.extend(entries_from_shape(fragment, slide, &info)?);
    }
    Ok(entries)
}

fn entries_from_notes_xml(xml: &str, slide: &SlideRef) -> CliResult<Vec<EntryData>> {
    let mut entries = Vec::new();
    for shape in element_ranges_by_local(xml, "sp")? {
        let fragment = &xml[shape.start..shape.end];
        let Some(tx_body) = direct_child(fragment, "txBody")? else {
            continue;
        };
        entries.extend(entries_from_text_body(
            &tx_body, slide, "notes", "notes", None, "",
        )?);
    }
    Ok(entries)
}

fn entries_from_shape(
    fragment: &str,
    slide: &SlideRef,
    info: &ShapeInfo,
) -> CliResult<Vec<EntryData>> {
    let Some(tx_body) = direct_child(fragment, "txBody")? else {
        return Ok(Vec::new());
    };
    let key = if info.key.is_empty() {
        "body"
    } else {
        info.key.as_str()
    };
    entries_from_text_body(
        &tx_body,
        slide,
        key,
        entry_type_for_key(key),
        info.id,
        &info.name,
    )
}

fn entries_from_text_body(
    tx_body: &str,
    slide: &SlideRef,
    placeholder_key: &str,
    entry_type: &str,
    shape_id: Option<i64>,
    shape_name: &str,
) -> CliResult<Vec<EntryData>> {
    let mut entries = Vec::new();
    for (paragraph_index, paragraph) in direct_children(tx_body)?
        .into_iter()
        .filter(|child| child.kind == "p")
        .enumerate()
    {
        let paragraph_fragment = &tx_body[paragraph.start..paragraph.end];
        let bullet_info = direct_child(paragraph_fragment, "pPr")?
            .as_deref()
            .map(parse_bullet_info);
        let mut run_index = 0_i64;
        for run in direct_children(paragraph_fragment)? {
            let run_fragment = &paragraph_fragment[run.start..run.end];
            let (source_text, segment_type, run_format) = match run.kind.as_str() {
                "r" => (
                    text_child_value(run_fragment)?,
                    "text".to_string(),
                    run_format_child(run_fragment)?,
                ),
                "br" => (
                    "\n".to_string(),
                    "break".to_string(),
                    run_format_child(run_fragment)?,
                ),
                "tab" => ("\t".to_string(), "tab".to_string(), None),
                "fld" => (
                    text_child_value(run_fragment)?,
                    "field".to_string(),
                    run_format_child(run_fragment)?,
                ),
                _ => continue,
            };
            let current_run_index = run_index;
            run_index += 1;
            if source_text.is_empty() {
                continue;
            }
            let id = format!(
                "slide:{}_{}_p{}_r{}",
                slide.slide_number - 1,
                placeholder_key,
                paragraph_index,
                current_run_index
            );
            entries.push(EntryData {
                id,
                entry_type: entry_type.to_string(),
                source_text,
                target_text: String::new(),
                slide_id: slide.slide_number - 1,
                slide_number: slide.slide_number,
                placeholder_key: placeholder_key.to_string(),
                shape_id,
                shape_name: shape_name.to_string(),
                paragraph_index: paragraph_index as i64,
                run_index: current_run_index,
                segment_type,
                bullet_info: bullet_info.clone(),
                run_format,
            });
        }
    }
    Ok(entries)
}

fn entry_to_json(entry: EntryData) -> Value {
    let mut object = Map::new();
    object.insert("id".to_string(), json!(entry.id));
    object.insert("type".to_string(), json!(entry.entry_type));
    object.insert("sourceText".to_string(), json!(entry.source_text));
    if !entry.target_text.is_empty() {
        object.insert("targetText".to_string(), json!(entry.target_text));
    }
    object.insert("slideId".to_string(), json!(entry.slide_id));
    object.insert("slideNumber".to_string(), json!(entry.slide_number));
    if !entry.placeholder_key.is_empty() {
        object.insert("placeholderKey".to_string(), json!(entry.placeholder_key));
    }
    if let Some(shape_id) = entry.shape_id {
        object.insert("shapeId".to_string(), json!(shape_id));
    }
    if !entry.shape_name.is_empty() {
        object.insert("shapeName".to_string(), json!(entry.shape_name));
    }
    object.insert("paragraphIndex".to_string(), json!(entry.paragraph_index));
    object.insert("runIndex".to_string(), json!(entry.run_index));
    if !entry.segment_type.is_empty() {
        object.insert("segmentType".to_string(), json!(entry.segment_type));
    }
    if let Some(bullet_info) = entry.bullet_info {
        object.insert("bulletInfo".to_string(), Value::Object(bullet_info));
    }
    if let Some(run_format) = entry.run_format {
        object.insert("runFormat".to_string(), Value::Object(run_format));
    }
    Value::Object(object)
}

fn entry_type_for_key(key: &str) -> &'static str {
    if key.starts_with("title") {
        "title"
    } else if key.starts_with("subtitle") {
        "subtitle"
    } else if key == "notes" {
        "notes"
    } else {
        "body"
    }
}

fn parse_shape_info(fragment: &str) -> CliResult<ShapeInfo> {
    let mut info = ShapeInfo::default();
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(false);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => match local_name(e.name().as_ref()) {
                "cNvPr" if info.id.is_none() => {
                    info.id = attr(&e, "id").and_then(|value| value.parse::<i64>().ok());
                    info.name = attr(&e, "name").unwrap_or_default();
                }
                "ph" if info.placeholder_type.is_empty() => {
                    info.placeholder_type = attr(&e, "type").unwrap_or_default();
                    info.placeholder_idx = attr(&e, "idx");
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    info.key = shape_key(&info);
    Ok(info)
}

fn shape_key(info: &ShapeInfo) -> String {
    match info.placeholder_type.as_str() {
        "ctrTitle" | "title" => return "title".to_string(),
        "subTitle" => return "subtitle".to_string(),
        "body" | "obj" => {
            return info
                .placeholder_idx
                .as_ref()
                .filter(|idx| !idx.is_empty())
                .map(|idx| format!("body:{idx}"))
                .unwrap_or_else(|| "body".to_string());
        }
        _ => {}
    }
    if info
        .name
        .to_ascii_lowercase()
        .contains("content placeholder")
        && let Some(idx) = &info.placeholder_idx
    {
        return format!("body:{idx}");
    }
    if !info.name.is_empty() {
        return info.name.clone();
    }
    info.id
        .map(|id| format!("shape:{id}"))
        .unwrap_or_else(|| "body".to_string())
}

fn direct_child(fragment: &str, wanted: &str) -> CliResult<Option<String>> {
    Ok(direct_children(fragment)?
        .into_iter()
        .find(|child| child.kind == wanted)
        .map(|child| fragment[child.start..child.end].to_string()))
}

fn direct_children(fragment: &str) -> CliResult<Vec<XmlNamedRange>> {
    let (open_end, _tag_name, close_start, self_closing) = xml_fragment_bounds(fragment)?;
    if self_closing {
        return Ok(Vec::new());
    }
    xml_direct_child_ranges(fragment, open_end + 1, close_start)
}

fn text_child_value(fragment: &str) -> CliResult<String> {
    let mut text = String::new();
    for child in direct_children(fragment)? {
        if child.kind == "t" {
            text.push_str(&xml_text_content(&fragment[child.start..child.end])?);
        }
    }
    Ok(text)
}

fn xml_text_content(fragment: &str) -> CliResult<String> {
    let (open_end, _tag_name, close_start, self_closing) = xml_fragment_bounds(fragment)?;
    if self_closing {
        return Ok(String::new());
    }
    Ok(crate::xml_util::xml_unescape(
        &fragment[open_end + 1..close_start],
    ))
}

fn run_format_child(fragment: &str) -> CliResult<Option<Map<String, Value>>> {
    Ok(direct_child(fragment, "rPr")?
        .as_deref()
        .map(parse_run_format))
}

fn parse_bullet_info(fragment: &str) -> Map<String, Value> {
    let mut info = Map::new();
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(false);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => match local_name(e.name().as_ref()) {
                "pPr" => {
                    if let Some(level) = attr(&e, "lvl").and_then(|value| value.parse::<i64>().ok())
                    {
                        info.insert("level".to_string(), json!(level));
                    }
                }
                "buNone" => {
                    info.insert("bulletMode".to_string(), json!("buNone"));
                }
                "buChar" => {
                    info.insert("bulletMode".to_string(), json!("buChar"));
                    if let Some(ch) = attr(&e, "char") {
                        info.insert("bulletCharacter".to_string(), json!(ch));
                    }
                }
                "buAutoNum" => {
                    info.insert("bulletMode".to_string(), json!("buAutoNum"));
                    if let Some(scheme) = attr(&e, "type") {
                        info.insert("autoNumberingScheme".to_string(), json!(scheme));
                    }
                }
                "buFont" => {
                    if let Some(typeface) = attr(&e, "typeface") {
                        info.insert("bulletFontFamily".to_string(), json!(typeface));
                    }
                }
                "buSzPts" => {
                    if let Some(size) = attr(&e, "val").and_then(|value| value.parse::<i64>().ok())
                    {
                        info.insert("bulletFontSize".to_string(), json!(size));
                    }
                }
                "srgbClr" if !info.contains_key("bulletColor") => {
                    if let Some(color) = attr(&e, "val") {
                        info.insert("bulletColor".to_string(), json!(color));
                    }
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    info
}

fn parse_run_format(fragment: &str) -> Map<String, Value> {
    let mut format = Map::new();
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(false);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => match local_name(e.name().as_ref()) {
                "rPr" => {
                    if attr(&e, "b").is_some() {
                        format.insert("bold".to_string(), json!(true));
                    }
                    if attr(&e, "i").is_some() {
                        format.insert("italic".to_string(), json!(true));
                    }
                    if let Some(underline) = attr(&e, "u") {
                        format.insert("underline".to_string(), json!(underline));
                    }
                    if let Some(strike) = attr(&e, "strike") {
                        format.insert("strike".to_string(), json!(strike));
                    }
                    if let Some(size) = attr(&e, "sz")
                        .and_then(|value| value.parse::<f64>().ok())
                        .map(|value| value / 100.0)
                    {
                        format.insert("fontSize".to_string(), json!(size));
                    }
                    if let Some(language) = attr(&e, "lang") {
                        format.insert("language".to_string(), json!(language));
                    }
                }
                "latin" | "ea" | "cs" if !format.contains_key("fontFamily") => {
                    if let Some(typeface) = attr(&e, "typeface") {
                        format.insert("fontFamily".to_string(), json!(typeface));
                    }
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    format
}

fn apply_entry_to_slide(
    file: &str,
    slide: &SlideRef,
    entry: &EntryData,
    overrides: &mut BTreeMap<String, String>,
) -> CliResult<()> {
    let xml = overrides
        .get(&slide.part)
        .cloned()
        .map(Ok)
        .unwrap_or_else(|| zip_text(file, &slide.part))?;
    let updated = apply_entry_to_slide_xml(&xml, entry)?;
    overrides.insert(slide.part.clone(), updated);
    Ok(())
}

fn apply_entry_to_notes(
    file: &str,
    slide: &SlideRef,
    entry: &EntryData,
    overrides: &mut BTreeMap<String, String>,
) -> CliResult<()> {
    let notes_part = slide.notes_part.as_ref().ok_or_else(|| {
        CliError::target_not_found(format!("notes not found for slide {}", slide.slide_number))
    })?;
    let xml = overrides
        .get(notes_part)
        .cloned()
        .map(Ok)
        .unwrap_or_else(|| zip_text(file, notes_part))?;
    let updated = apply_entry_to_notes_xml(&xml, entry)?;
    overrides.insert(notes_part.clone(), updated);
    Ok(())
}

fn apply_entry_to_slide_xml(xml: &str, entry: &EntryData) -> CliResult<String> {
    for shape in element_ranges_by_local(xml, "sp")? {
        let fragment = &xml[shape.start..shape.end];
        let info = parse_shape_info(fragment)?;
        if !shape_matches(&info, entry) {
            continue;
        }
        let updated_fragment = apply_entry_to_shape_fragment(fragment, entry)?;
        return Ok(replace_xml_span(
            xml,
            shape.start,
            shape.end,
            &updated_fragment,
        ));
    }
    Err(CliError::target_not_found(format!(
        "shape not found: {}",
        entry.placeholder_key
    )))
}

fn apply_entry_to_notes_xml(xml: &str, entry: &EntryData) -> CliResult<String> {
    for shape in element_ranges_by_local(xml, "sp")? {
        let fragment = &xml[shape.start..shape.end];
        if direct_child(fragment, "txBody")?.is_none() {
            continue;
        }
        if let Ok(updated_fragment) = apply_entry_to_shape_fragment(fragment, entry) {
            return Ok(replace_xml_span(
                xml,
                shape.start,
                shape.end,
                &updated_fragment,
            ));
        }
    }
    Err(CliError::target_not_found("notes text not found"))
}

fn shape_matches(info: &ShapeInfo, entry: &EntryData) -> bool {
    if !entry.placeholder_key.is_empty()
        && (info.key == entry.placeholder_key
            || info.name == entry.placeholder_key
            || info
                .id
                .is_some_and(|id| format!("shape:{id}") == entry.placeholder_key))
    {
        return true;
    }
    if let (Some(left), Some(right)) = (info.id, entry.shape_id)
        && left == right
    {
        return true;
    }
    !entry.shape_name.is_empty() && info.name == entry.shape_name
}

fn apply_entry_to_shape_fragment(fragment: &str, entry: &EntryData) -> CliResult<String> {
    let tx_body = direct_children(fragment)?
        .into_iter()
        .find(|child| child.kind == "txBody")
        .ok_or_else(|| CliError::target_not_found("shape text body not found"))?;
    let tx_body_fragment = &fragment[tx_body.start..tx_body.end];
    let updated_tx_body = apply_entry_to_text_body(tx_body_fragment, entry)?;
    Ok(replace_xml_span(
        fragment,
        tx_body.start,
        tx_body.end,
        &updated_tx_body,
    ))
}

fn apply_entry_to_text_body(tx_body: &str, entry: &EntryData) -> CliResult<String> {
    let paragraphs = direct_children(tx_body)?
        .into_iter()
        .filter(|child| child.kind == "p")
        .collect::<Vec<_>>();
    let paragraph = paragraphs
        .get(entry.paragraph_index as usize)
        .ok_or_else(|| CliError::target_not_found("paragraph index out of range"))?;
    let paragraph_fragment = &tx_body[paragraph.start..paragraph.end];
    let updated_paragraph = apply_entry_to_paragraph(paragraph_fragment, entry)?;
    Ok(replace_xml_span(
        tx_body,
        paragraph.start,
        paragraph.end,
        &updated_paragraph,
    ))
}

fn apply_entry_to_paragraph(paragraph: &str, entry: &EntryData) -> CliResult<String> {
    let runs = direct_children(paragraph)?
        .into_iter()
        .filter(|child| matches!(child.kind.as_str(), "r" | "br" | "tab" | "fld"))
        .collect::<Vec<_>>();
    let run = runs
        .get(entry.run_index as usize)
        .ok_or_else(|| CliError::target_not_found("run index out of range"))?;
    let run_fragment = &paragraph[run.start..run.end];
    let updated_run = match run.kind.as_str() {
        "br" => {
            return Err(CliError::unexpected(format!(
                "cannot apply translation to break element (run {})",
                entry.run_index
            )));
        }
        "tab" => run_fragment.to_string(),
        "r" | "fld" => set_text_child_value(run_fragment, &entry.target_text)?,
        _ => run_fragment.to_string(),
    };
    Ok(replace_xml_span(
        paragraph,
        run.start,
        run.end,
        &updated_run,
    ))
}

fn set_text_child_value(fragment: &str, text: &str) -> CliResult<String> {
    if let Some(t_range) = direct_children(fragment)?
        .into_iter()
        .find(|child| child.kind == "t")
    {
        let t_fragment = &fragment[t_range.start..t_range.end];
        let updated_t = replace_text_element(t_fragment, text)?;
        return Ok(replace_xml_span(
            fragment,
            t_range.start,
            t_range.end,
            &updated_t,
        ));
    }

    let (open_end, tag_name, close_start, self_closing) = xml_fragment_bounds(fragment)?;
    if self_closing {
        return Ok(fragment.to_string());
    }
    let prefix = xml_tag_prefix(&tag_name);
    let t_tag = if prefix.is_empty() {
        "t".to_string()
    } else {
        format!("{prefix}:t")
    };
    let xml_space = if needs_xml_space_preserve(text) {
        r#" xml:space="preserve""#
    } else {
        ""
    };
    let insertion = format!("<{t_tag}{xml_space}>{}</{t_tag}>", xml_escape(text));
    Ok(replace_xml_span(
        fragment,
        open_end + 1,
        close_start,
        &format!("{}{}", &fragment[open_end + 1..close_start], insertion),
    ))
}

fn replace_text_element(fragment: &str, text: &str) -> CliResult<String> {
    let (open_end, tag_name, _close_start, _self_closing) = xml_fragment_bounds(fragment)?;
    let mut open_tag = fragment[..=open_end].to_string();
    if open_tag.trim_end().ends_with("/>")
        && let Some(slash) = open_tag.rfind('/')
    {
        open_tag.replace_range(slash..=slash, "");
    }
    if needs_xml_space_preserve(text) && !open_tag.contains("xml:space=") {
        let insert_at = open_tag
            .rfind('>')
            .ok_or_else(|| CliError::unexpected("invalid text element"))?;
        open_tag.insert_str(insert_at, r#" xml:space="preserve""#);
    }
    Ok(format!("{open_tag}{}</{tag_name}>", xml_escape(text)))
}

fn warn_apply(warnings: &mut Vec<String>, msg: String) {
    eprintln!("WARNING: {msg}");
    warnings.push(msg);
}

fn element_ranges_by_local(xml: &str, wanted: &str) -> CliResult<Vec<XmlNamedRange>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut stack: Vec<(String, usize)> = Vec::new();
    let mut ranges = Vec::new();
    loop {
        let start = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                stack.push((local_name(e.name().as_ref()).to_string(), start));
            }
            Ok(Event::Empty(e)) => {
                if local_name(e.name().as_ref()) == wanted {
                    ranges.push(XmlNamedRange {
                        start,
                        end: reader.buffer_position() as usize,
                        kind: wanted.to_string(),
                    });
                }
            }
            Ok(Event::End(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                if let Some(position) = stack.iter().rposition(|(local, _)| local == &name) {
                    let (local, range_start) = stack.remove(position);
                    if local == wanted {
                        ranges.push(XmlNamedRange {
                            start: range_start,
                            end: reader.buffer_position() as usize,
                            kind: local,
                        });
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(ranges)
}
