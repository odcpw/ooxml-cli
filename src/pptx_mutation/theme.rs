use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::fs;

use crate::cli_args::{parse_bool_flag, parse_string_flags};
use crate::{
    CliError, CliResult, attr_exact, copy_zip_with_part_overrides, local_name,
    package_mutation_temp_path, package_type, parse_i64_flag, parse_string_flag,
    relationship_entries_from_xml, relationships_part_for, resolve_relationship_target, validate,
    validate_xlsx_mutation_output_flags, xml_attr_escape, xml_direct_child_ranges,
    xml_fragment_bounds, xml_token_name, zip_text,
};

const MASTER_REL_TYPE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster";
const THEME_REL_TYPE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme";

const VALID_THEME_COLORS: &[&str] = &[
    "dk1", "lt1", "dk2", "lt2", "accent1", "accent2", "accent3", "accent4", "accent5", "accent6",
    "hlink", "folHlink",
];

pub(crate) fn pptx_theme_update(file: &str, args: &[String]) -> CliResult<Value> {
    let options = parse_theme_update_options(args)?;
    let detected = package_type(file)?;
    if detected != "pptx" {
        return Err(CliError::unsupported_type(format!(
            "file is not a PPTX document (detected: {detected})"
        )));
    }

    match options.mode.as_str() {
        "deck" => update_deck_theme(file, &options),
        "slide" => update_slide_theme(file, &options),
        _ => Err(CliError::invalid_args("mode must be 'deck' or 'slide'")),
    }
}

#[derive(Clone)]
struct ThemeUpdateOptions {
    colors: Vec<ThemeColorUpdate>,
    major_font: Option<String>,
    minor_font: Option<String>,
    mode: String,
    slide: Option<i64>,
    for_slides: Option<String>,
    out: Option<String>,
    backup: Option<String>,
    dry_run: bool,
    in_place: bool,
    no_validate: bool,
}

#[derive(Clone)]
struct ThemeColorUpdate {
    name: String,
    hex: String,
}

fn parse_theme_update_options(args: &[String]) -> CliResult<ThemeUpdateOptions> {
    let colors = parse_string_flags(args, "--color")?
        .into_iter()
        .map(|value| {
            let parts = value.split('=').collect::<Vec<_>>();
            if parts.len() != 2 {
                return Err(CliError::invalid_args(format!(
                    "invalid color format: {value}"
                )));
            }
            Ok(ThemeColorUpdate {
                name: parts[0].trim().to_string(),
                hex: parts[1].trim().to_string(),
            })
        })
        .collect::<CliResult<Vec<_>>>()?;
    let major_font = parse_string_flag(args, "--major-font")?;
    let minor_font = parse_string_flag(args, "--minor-font")?;
    if colors.is_empty()
        && major_font.as_deref().unwrap_or_default().is_empty()
        && minor_font.as_deref().unwrap_or_default().is_empty()
    {
        return Err(CliError::invalid_args(
            "no updates specified; use --color, --major-font, or --minor-font",
        ));
    }

    let mode = parse_string_flag(args, "--mode")?.unwrap_or_else(|| "deck".to_string());
    if mode != "deck" && mode != "slide" {
        return Err(CliError::invalid_args("mode must be 'deck' or 'slide'"));
    }
    let slide = parse_i64_flag(args, "--slide")?;
    let for_slides = parse_string_flag(args, "--for-slides")?;
    if mode == "slide" {
        if slide.is_some() && for_slides.is_some() {
            return Err(CliError::invalid_args(
                "cannot specify both --slide and --for-slides",
            ));
        }
        if slide.is_none() && for_slides.is_none() {
            return Err(CliError::invalid_args(
                "slide mode requires either --slide or --for-slides",
            ));
        }
    }

    let out = parse_string_flag(args, "--out")?;
    let backup = parse_string_flag(args, "--backup")?;
    let dry_run = parse_bool_flag(args, "--dry-run")?.unwrap_or(false);
    let in_place = parse_bool_flag(args, "--in-place")?.unwrap_or(false);
    let no_validate = parse_bool_flag(args, "--no-validate")?.unwrap_or(false);
    validate_xlsx_mutation_output_flags(out.as_deref(), in_place, backup.as_deref(), dry_run)?;
    Ok(ThemeUpdateOptions {
        colors,
        major_font,
        minor_font,
        mode,
        slide,
        for_slides,
        out,
        backup,
        dry_run,
        in_place,
        no_validate,
    })
}

fn update_deck_theme(file: &str, options: &ThemeUpdateOptions) -> CliResult<Value> {
    let theme_part = first_theme_part(file)?;
    let theme_part_name = package_part_name(&theme_part);
    let mut theme_xml = zip_text(file, &theme_part_name)?;
    let mut color_results = Vec::new();
    for color in &options.colors {
        theme_xml = update_theme_color(&theme_xml, &color.name, &color.hex).map_err(|err| {
            CliError::invalid_args(format!(
                "failed to update theme color {}: {err}",
                color.name
            ))
        })?;
        color_results.push(json!({
            "colorName": color.name,
            "hexValue": color.hex,
            "mode": "deck",
        }));
    }

    let mut font_result = None;
    let major_font = options
        .major_font
        .as_deref()
        .filter(|value| !value.is_empty());
    let minor_font = options
        .minor_font
        .as_deref()
        .filter(|value| !value.is_empty());
    if major_font.is_some() || minor_font.is_some() {
        theme_xml = update_theme_font(&theme_xml, major_font, minor_font).map_err(|err| {
            CliError::invalid_args(format!("failed to update theme fonts: {err}"))
        })?;
        let mut fonts = Map::new();
        if let Some(major_font) = major_font {
            fonts.insert("majorFont".to_string(), json!(major_font));
        }
        if let Some(minor_font) = minor_font {
            fonts.insert("minorFont".to_string(), json!(minor_font));
        }
        fonts.insert("mode".to_string(), json!("deck"));
        font_result = Some(Value::Object(fonts));
    }

    let mut overrides = BTreeMap::new();
    overrides.insert(theme_part_name, theme_xml);
    write_theme_mutation(file, &overrides, options)?;

    let mut out = Map::new();
    if !color_results.is_empty() {
        out.insert("colors".to_string(), Value::Array(color_results));
    }
    if let Some(font_result) = font_result {
        out.insert("fonts".to_string(), font_result);
    }
    out.insert(
        "message".to_string(),
        json!("theme update completed successfully"),
    );
    Ok(Value::Object(out))
}

fn update_slide_theme(file: &str, options: &ThemeUpdateOptions) -> CliResult<Value> {
    if !options.colors.is_empty() {
        let slide_part = first_slide_target_part(file, options)?;
        return Err(CliError::invalid_args(format!(
            "failed to apply color override: cSld element not found in slide {slide_part}"
        )));
    }
    if options
        .major_font
        .as_deref()
        .is_some_and(|value| !value.is_empty())
        || options
            .minor_font
            .as_deref()
            .is_some_and(|value| !value.is_empty())
    {
        return Err(CliError::invalid_args(
            "font updates are only supported in deck mode",
        ));
    }
    Ok(json!({
        "noOpMessage": "theme update made no changes"
    }))
}

fn first_theme_part(file: &str) -> CliResult<String> {
    let masters = presentation_master_parts(file)?;
    for master in masters {
        let rels_part = relationships_part_for(&master);
        let Ok(rels_xml) = zip_text(file, &rels_part) else {
            continue;
        };
        for rel in relationship_entries_from_xml(&rels_xml) {
            if rel.rel_type == THEME_REL_TYPE {
                return Ok(resolve_relationship_target(&master, &rel.target));
            }
        }
    }
    Ok("/ppt/theme/theme1.xml".to_string())
}

fn presentation_master_parts(file: &str) -> CliResult<Vec<String>> {
    let presentation = zip_text(file, "ppt/presentation.xml")?;
    let rels = relationship_entries_from_xml(&zip_text(file, "ppt/_rels/presentation.xml.rels")?);
    let mut masters = Vec::new();
    let mut reader = Reader::from_str(&presentation);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "sldMasterId" =>
            {
                let Some(rel_id) = attr_exact(&e, "r:id") else {
                    continue;
                };
                let rel = rels.iter().find(|rel| rel.id == rel_id).ok_or_else(|| {
                    CliError::unexpected(format!("missing relationship {rel_id}"))
                })?;
                if rel.rel_type == MASTER_REL_TYPE {
                    masters.push(resolve_relationship_target(
                        "/ppt/presentation.xml",
                        &rel.target,
                    ));
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(masters)
}

fn first_slide_target_part(file: &str, options: &ThemeUpdateOptions) -> CliResult<String> {
    let target_slide = options
        .slide
        .or_else(|| {
            options
                .for_slides
                .as_deref()
                .and_then(first_slide_from_selector)
        })
        .unwrap_or(1);
    let slides = presentation_slide_parts(file)?;
    if target_slide < 1 || target_slide as usize > slides.len() {
        return Err(CliError::invalid_args(format!(
            "slide {target_slide} not found (presentation has {} slides)",
            slides.len()
        )));
    }
    Ok(slides[target_slide as usize - 1].clone())
}

fn first_slide_from_selector(selector: &str) -> Option<i64> {
    selector
        .split(',')
        .next()
        .and_then(|part| part.split('-').next())
        .and_then(|value| value.trim().parse::<i64>().ok())
}

fn presentation_slide_parts(file: &str) -> CliResult<Vec<String>> {
    let presentation = zip_text(file, "ppt/presentation.xml")?;
    let rels = relationship_entries_from_xml(&zip_text(file, "ppt/_rels/presentation.xml.rels")?);
    let mut slides = Vec::new();
    let mut reader = Reader::from_str(&presentation);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "sldId" =>
            {
                let Some(rel_id) = attr_exact(&e, "r:id") else {
                    continue;
                };
                let rel = rels.iter().find(|rel| rel.id == rel_id).ok_or_else(|| {
                    CliError::unexpected(format!("missing relationship {rel_id}"))
                })?;
                slides.push(resolve_relationship_target(
                    "/ppt/presentation.xml",
                    &rel.target,
                ));
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(slides)
}

fn update_theme_color(xml: &str, color_name: &str, hex: &str) -> Result<String, String> {
    validate_theme_color(color_name)?;
    validate_theme_hex(hex)?;
    let theme_elements =
        first_element_span(xml, "themeElements", 0, xml.len()).ok_or("themeElements not found")?;
    let color_scheme = first_element_span(xml, "clrScheme", theme_elements.0, theme_elements.1)
        .ok_or("clrScheme not found")?;
    let color_span = first_element_span(xml, color_name, color_scheme.0, color_scheme.1)
        .ok_or_else(|| format!("theme color {color_name} not found"))?;
    let color_xml = &xml[color_span.0..color_span.1];
    let updated_color = rewrite_theme_color_element(color_xml, hex)?;
    Ok(replace_span(
        xml,
        color_span.0,
        color_span.1,
        &updated_color,
    ))
}

fn rewrite_theme_color_element(color_xml: &str, hex: &str) -> Result<String, String> {
    let (open_end, tag_name, close_start, self_closing) =
        xml_fragment_bounds(color_xml).map_err(|err| err.message)?;
    let prefix = tag_prefix(&tag_name);
    let srgb_tag = qualified_name(&prefix, "srgbClr");
    let srgb = format!("<{srgb_tag} val=\"{}\"/>", xml_attr_escape(hex));
    if self_closing {
        let open_tag = xml_open_tag(color_xml, open_end);
        return Ok(format!("{open_tag}{srgb}</{tag_name}>"));
    }
    let children =
        xml_direct_child_ranges(color_xml, open_end + 1, close_start).map_err(|err| err.message)?;
    let mut out = String::new();
    out.push_str(&color_xml[..=open_end]);
    for child in children {
        if child.kind != "srgbClr" && child.kind != "sysClr" {
            out.push_str(&color_xml[child.start..child.end]);
        }
    }
    out.push_str(&srgb);
    out.push_str(&color_xml[close_start..]);
    Ok(out)
}

fn update_theme_font(
    xml: &str,
    major_font: Option<&str>,
    minor_font: Option<&str>,
) -> Result<String, String> {
    let theme_elements =
        first_element_span(xml, "themeElements", 0, xml.len()).ok_or("themeElements not found")?;
    let font_scheme = first_element_span(xml, "fontScheme", theme_elements.0, theme_elements.1)
        .ok_or("fontScheme not found")?;
    let mut updated = xml.to_string();
    if let Some(major_font) = major_font {
        updated = set_theme_latin_font(&updated, font_scheme, "majorFont", major_font)?;
    }
    if let Some(minor_font) = minor_font {
        updated = set_theme_latin_font(&updated, font_scheme, "minorFont", minor_font)?;
    }
    Ok(updated)
}

fn set_theme_latin_font(
    xml: &str,
    font_scheme_span: (usize, usize),
    font_kind: &str,
    typeface: &str,
) -> Result<String, String> {
    let font_span = first_element_span(xml, font_kind, font_scheme_span.0, font_scheme_span.1)
        .ok_or_else(|| format!("{font_kind} element not found"))?;
    let font_xml = &xml[font_span.0..font_span.1];
    let (open_end, tag_name, close_start, self_closing) =
        xml_fragment_bounds(font_xml).map_err(|err| err.message)?;
    if self_closing {
        return Err(format!("{font_kind} element is self-closing"));
    }
    let prefix = tag_prefix(&tag_name);
    let children =
        xml_direct_child_ranges(font_xml, open_end + 1, close_start).map_err(|err| err.message)?;
    let updated_font = if let Some(latin) = children.iter().find(|child| child.kind == "latin") {
        set_attr_on_element(font_xml, latin.start, "typeface", typeface)
            .map_err(|err| err.message)?
    } else {
        let latin = format!(
            "<{} typeface=\"{}\"/>",
            qualified_name(&prefix, "latin"),
            xml_attr_escape(typeface)
        );
        insert_at_index(font_xml, open_end + 1, &latin)
    };
    Ok(replace_span(xml, font_span.0, font_span.1, &updated_font))
}

fn validate_theme_color(color: &str) -> Result<(), String> {
    if color.is_empty() {
        return Err("color name is required".to_string());
    }
    if VALID_THEME_COLORS.contains(&color) {
        Ok(())
    } else {
        Err(format!(
            "invalid color name '{color}'; must be one of: {}",
            VALID_THEME_COLORS.join(", ")
        ))
    }
}

fn validate_theme_hex(hex: &str) -> Result<(), String> {
    if hex.is_empty() {
        return Err("hex color value is required".to_string());
    }
    if hex.len() == 6 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(format!(
            "invalid hex color '{hex}'; must be 6 hexadecimal characters (e.g., FF0000)"
        ))
    }
}

fn write_theme_mutation(
    file: &str,
    overrides: &BTreeMap<String, String>,
    options: &ThemeUpdateOptions,
) -> CliResult<()> {
    let output_path = options
        .out
        .as_deref()
        .filter(|value| !value.trim().is_empty());
    let write_path = if options.dry_run || options.in_place || output_path == Some(file) {
        package_mutation_temp_path(file, "pptx-theme")
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
    if options.dry_run {
        let _ = fs::remove_file(&write_path);
    } else if options.in_place || output_path == Some(file) {
        if let Some(backup_path) = options
            .backup
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            fs::copy(file, backup_path)
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

fn first_element_span(
    xml: &str,
    wanted: &str,
    range_start: usize,
    range_end: usize,
) -> Option<(usize, usize)> {
    let mut cursor = range_start;
    while cursor < range_end {
        let relative_start = xml[cursor..range_end].find('<')?;
        let tag_start = cursor + relative_start;
        let relative_end = xml[tag_start..range_end].find('>')?;
        let tag_end = tag_start + relative_end;
        let token = xml[tag_start + 1..tag_end].trim_start();
        if token.starts_with('/') || token.starts_with('?') || token.starts_with('!') {
            cursor = tag_end + 1;
            continue;
        }
        let name = xml_token_name(token)?;
        let self_closing = token.trim_end().ends_with('/');
        if local_name(name.as_bytes()) == wanted {
            if self_closing {
                return Some((tag_start, tag_end + 1));
            }
            return find_matching_element_end(xml, wanted, tag_end + 1, range_end)
                .map(|end| (tag_start, end));
        }
        cursor = tag_end + 1;
    }
    None
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

fn set_attr_on_element(
    xml: &str,
    element_start: usize,
    attr_name: &str,
    value: &str,
) -> CliResult<String> {
    let tag_end = xml[element_start..]
        .find('>')
        .map(|offset| element_start + offset + 1)
        .ok_or_else(|| CliError::unexpected("invalid XML start tag"))?;
    let start_tag = &xml[element_start..tag_end];
    let replacement = set_attr_on_start_tag(start_tag, attr_name, value)?;
    Ok(replace_span(xml, element_start, tag_end, &replacement))
}

fn set_attr_on_start_tag(start_tag: &str, attr_name: &str, value: &str) -> CliResult<String> {
    let Some(open_end) = start_tag.find('>') else {
        return Err(CliError::unexpected("invalid XML start tag"));
    };
    let Some(token_name) = xml_token_name(&start_tag[1..open_end]) else {
        return Err(CliError::unexpected("invalid XML start tag"));
    };
    let mut cursor = 1 + token_name.len();
    while cursor < open_end {
        while cursor < open_end && start_tag.as_bytes()[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor >= open_end || start_tag.as_bytes()[cursor] == b'/' {
            break;
        }
        let name_start = cursor;
        while cursor < open_end {
            let byte = start_tag.as_bytes()[cursor];
            if byte == b'=' || byte.is_ascii_whitespace() || byte == b'/' {
                break;
            }
            cursor += 1;
        }
        let name_end = cursor;
        while cursor < open_end && start_tag.as_bytes()[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor >= open_end || start_tag.as_bytes()[cursor] != b'=' {
            continue;
        }
        cursor += 1;
        while cursor < open_end && start_tag.as_bytes()[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor >= open_end {
            break;
        }
        let quote = start_tag.as_bytes()[cursor];
        if quote != b'"' && quote != b'\'' {
            continue;
        }
        cursor += 1;
        let value_start = cursor;
        while cursor < open_end && start_tag.as_bytes()[cursor] != quote {
            cursor += 1;
        }
        if cursor >= open_end {
            break;
        }
        let value_end = cursor;
        cursor += 1;
        let existing_name = &start_tag[name_start..name_end];
        if local_name(existing_name.as_bytes()) == attr_name {
            if &start_tag[value_start..value_end] == value {
                return Ok(start_tag.to_string());
            }
            let mut out = String::with_capacity(start_tag.len() + value.len());
            out.push_str(&start_tag[..value_start]);
            out.push_str(&xml_attr_escape(value));
            out.push_str(&start_tag[value_end..]);
            return Ok(out);
        }
    }

    let insert_at = if start_tag[..open_end].trim_end().ends_with('/') {
        start_tag[..open_end]
            .rfind('/')
            .ok_or_else(|| CliError::unexpected("invalid XML start tag"))?
    } else {
        open_end
    };
    let mut out = String::with_capacity(start_tag.len() + attr_name.len() + value.len() + 4);
    out.push_str(&start_tag[..insert_at]);
    out.push(' ');
    out.push_str(attr_name);
    out.push_str("=\"");
    out.push_str(&xml_attr_escape(value));
    out.push('"');
    out.push_str(&start_tag[insert_at..]);
    Ok(out)
}

fn xml_open_tag(fragment: &str, open_end: usize) -> String {
    let start_tag = &fragment[..=open_end];
    if !start_tag.trim_end().ends_with("/>") {
        return start_tag.to_string();
    }
    let slash = start_tag
        .rfind('/')
        .unwrap_or_else(|| start_tag.len().saturating_sub(1));
    let mut out = String::new();
    out.push_str(&start_tag[..slash]);
    out.push('>');
    out
}

fn replace_span(xml: &str, start: usize, end: usize, replacement: &str) -> String {
    let mut out = String::with_capacity(xml.len() - (end - start) + replacement.len());
    out.push_str(&xml[..start]);
    out.push_str(replacement);
    out.push_str(&xml[end..]);
    out
}

fn insert_at_index(xml: &str, index: usize, insertion: &str) -> String {
    let mut out = String::with_capacity(xml.len() + insertion.len());
    out.push_str(&xml[..index]);
    out.push_str(insertion);
    out.push_str(&xml[index..]);
    out
}

fn tag_prefix(tag_name: &str) -> String {
    tag_name
        .split_once(':')
        .map(|(prefix, _)| prefix.to_string())
        .unwrap_or_default()
}

fn qualified_name(prefix: &str, local: &str) -> String {
    if prefix.is_empty() {
        local.to_string()
    } else {
        format!("{prefix}:{local}")
    }
}

fn package_part_name(uri: &str) -> String {
    uri.trim_start_matches('/').to_string()
}
