use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

use super::{DesignConfig, DesignFinding, finding, fixed_output_path, location};
use crate::palette::Srgb;
use crate::{
    CliError, CliResult, append_xml_text_event, attr, command_arg, is_xml_text_event, local_name,
    pptx_shapes_show, pptx_slides_list, pptx_validate_layout, zip_bytes, zip_text,
};

const EMU_PER_PIXEL_96_DPI: i64 = 9_525;

#[derive(Default)]
struct ShapeStyle {
    id: u32,
    text: String,
    font_sizes: Vec<f64>,
    fonts: BTreeSet<String>,
    colors: BTreeSet<String>,
    alt_text: String,
}

#[derive(Default)]
struct ThemePresentation {
    colors: BTreeMap<String, String>,
    fonts: BTreeSet<String>,
}

pub(super) fn analyze(
    file: &str,
    entries: &[String],
    config: &DesignConfig,
) -> CliResult<Vec<DesignFinding>> {
    let theme = scan_theme(file, entries)?;
    let slide_listing = pptx_slides_list(file)?;
    let slides = slide_listing["slides"]
        .as_array()
        .ok_or_else(|| CliError::unexpected("PPTX slide readback did not return a slides array"))?;
    let layout_report = pptx_validate_layout(file)?;
    let layout_slides = layout_report["slideReports"].as_array();
    let out = fixed_output_path(file, "design-fixed");
    let mut findings = Vec::new();
    let mut title_positions = BTreeMap::<String, (u32, [i64; 4])>::new();

    for (index, slide) in slides.iter().enumerate() {
        let slide_number = (index + 1) as u32;
        let part = slide["partUri"]
            .as_str()
            .unwrap_or_default()
            .trim_start_matches('/');
        let xml = zip_text(file, part)?;
        let styles = scan_slide_styles(&xml)?;
        let shapes_report = pptx_shapes_show(file, slide_number, true, true)?;
        let shapes = shapes_report["shapes"].as_array().ok_or_else(|| {
            CliError::unexpected("PPTX shape readback did not return a shapes array")
        })?;
        let layout_name = shapes_report["layoutPartUri"]
            .as_str()
            .or_else(|| shapes_report["layoutName"].as_str())
            .unwrap_or("unresolved-layout");
        let layout_xml = shapes_report["layoutPartUri"]
            .as_str()
            .and_then(|part| zip_text(file, part.trim_start_matches('/')).ok());
        let background = resolved_background(&xml, layout_xml.as_deref(), &theme.colors);
        let minimum_font = config.threshold("pptx.minimumFontPoints", 12.0);
        let minimum_contrast = config.threshold("pptx.minimumTextContrast", 4.5);
        let mut title = None::<&Value>;
        let mut outside_fonts = BTreeSet::new();

        for shape in shapes {
            let shape_id = shape["shapeId"].as_u64().unwrap_or_default() as u32;
            let selector = shape["primarySelector"]
                .as_str()
                .map(ToString::to_string)
                .unwrap_or_else(|| format!("shape:{shape_id}"));
            let style = styles.get(&shape_id);
            let text = shape_text(shape);
            let placeholder_role = shape["placeholder"]["role"].as_str().unwrap_or_default();
            if matches!(placeholder_role, "title" | "centerTitle") {
                if !text.trim().is_empty() {
                    title = Some(shape);
                }
            } else if title.is_none()
                && !text.trim().is_empty()
                && bounds(shape).is_some_and(|bounds| bounds[1] < 1_371_600)
            {
                title = Some(shape);
            }

            if shape.get("placeholder").is_some() && text.trim().is_empty() {
                findings.push(finding(
                    "PPTX_EMPTY_PLACEHOLDER",
                    format!("Slide {slide_number} leaves placeholder {selector} empty"),
                    shape_location(slide_number, part, shape_id),
                    format!(
                        "ooxml --json pptx shapes delete {} --slide {slide_number} --target {} --out {}",
                        command_arg(file), command_arg(&selector), command_arg(&out)
                    ),
                    None,
                ));
            }

            if let Some(style) = style {
                if let Some(size) = style
                    .font_sizes
                    .iter()
                    .copied()
                    .filter(|size| *size < minimum_font)
                    .min_by(f64::total_cmp)
                {
                    findings.push(finding(
                        "PPTX_FONT_TOO_SMALL",
                        format!("Slide {slide_number} shape {selector} uses {size:.1} pt text"),
                        shape_location(slide_number, part, shape_id),
                        format!(
                            "ooxml --json pptx shapes delete {} --slide {slide_number} --target {} --out {}",
                            command_arg(file), command_arg(&selector), command_arg(&out)
                        ),
                        Some(json!({"minimumPoints": minimum_font, "observedPoints": size})),
                    ));
                }
                outside_fonts.extend(
                    style
                        .fonts
                        .iter()
                        .filter(|font| !is_theme_font(font, &theme.fonts))
                        .cloned(),
                );
                for color in &style.colors {
                    let foreground = resolve_color(color, &theme.colors).unwrap_or(Srgb::BLACK);
                    // Converting both colors to OKLCH is deliberate: the evidence records the
                    // perceptual coordinates while WCAG contrast remains luminance-defined.
                    let foreground_oklch = foreground.to_oklch();
                    let background_oklch = background.to_oklch();
                    let ratio = foreground.contrast_ratio(background);
                    if ratio < minimum_contrast {
                        findings.push(finding(
                            "PPTX_TEXT_CONTRAST",
                            format!("Slide {slide_number} shape {selector} has {ratio:.2}:1 text contrast"),
                            shape_location(slide_number, part, shape_id),
                            format!(
                                "ooxml --json pptx text set {} --slide {slide_number} --target {} --color 000000 --out {}",
                                command_arg(file), command_arg(&selector), command_arg(&out)
                            ),
                            Some(json!({
                                "contrastRatio": ratio,
                                "minimum": minimum_contrast,
                                "foreground": foreground.to_hex(),
                                "background": background.to_hex(),
                                "foregroundOklch": {"l": foreground_oklch.l, "c": foreground_oklch.c, "h": foreground_oklch.h},
                                "backgroundOklch": {"l": background_oklch.l, "c": background_oklch.c, "h": background_oklch.h},
                            })),
                        ));
                    }
                }
            }

            let paragraphs = shape["paragraphs"].as_array();
            let bullet_count = paragraphs
                .into_iter()
                .flatten()
                .filter(|paragraph| paragraph["bullet"].as_bool() == Some(true))
                .count();
            let maximum_level = paragraphs
                .into_iter()
                .flatten()
                .filter_map(|paragraph| paragraph["level"].as_u64())
                .max()
                .unwrap_or(0);
            if bullet_count > config.threshold("pptx.maximumBullets", 7.0) as usize
                || maximum_level >= config.threshold("pptx.maximumBulletLevels", 2.0) as u64
            {
                findings.push(finding(
                    "PPTX_BULLET_OVERLOAD",
                    format!("Slide {slide_number} shape {selector} has {bullet_count} bullets and {} levels", maximum_level + 1),
                    shape_location(slide_number, part, shape_id),
                    format!(
                        "ooxml --json pptx shapes delete {} --slide {slide_number} --target {} --out {}",
                        command_arg(file), command_arg(&selector), command_arg(&out)
                    ),
                    Some(json!({"bulletCount": bullet_count, "levels": maximum_level + 1})),
                ));
            }

            if shape["targetKind"].as_str() == Some("picture") {
                let alt_text = style
                    .map(|style| style.alt_text.as_str())
                    .unwrap_or_default();
                if alt_text.trim().is_empty() {
                    findings.push(finding(
                        "PPTX_MISSING_ALT_TEXT",
                        format!("Slide {slide_number} picture {selector} has no title or description"),
                        shape_location(slide_number, part, shape_id),
                        format!(
                            "ooxml --json pptx shapes delete {} --slide {slide_number} --target {} --out {}",
                            command_arg(file), command_arg(&selector), command_arg(&out)
                        ),
                        None,
                    ));
                }
                if let Some(evidence) = image_scale_evidence(file, shape)? {
                    findings.push(finding(
                        "PPTX_IMAGE_SCALE",
                        format!("Slide {slide_number} picture {selector} is upscaled or aspect-distorted"),
                        shape_location(slide_number, part, shape_id),
                        format!(
                            "ooxml --json pptx shapes delete {} --slide {slide_number} --target {} --out {}",
                            command_arg(file), command_arg(&selector), command_arg(&out)
                        ),
                        Some(evidence),
                    ));
                }
            }
        }

        if outside_fonts.len() > 1 {
            let theme_font = outside_fonts.first().map(String::as_str).unwrap_or("Aptos");
            findings.push(finding(
                "PPTX_FONT_OUTSIDE_THEME",
                format!("Slide {slide_number} uses multiple font families outside the theme"),
                location(&[
                    ("slide", json!(slide_number)),
                    ("part", json!(format!("/{part}"))),
                ]),
                format!(
                    "ooxml --json pptx theme update {} --major-font {} --minor-font {} --out {}",
                    command_arg(file),
                    command_arg(theme_font),
                    command_arg(theme_font),
                    command_arg(&out)
                ),
                Some(json!({"fonts": outside_fonts, "themeFonts": theme.fonts})),
            ));
        }

        if title.is_none() {
            findings.push(finding(
                "PPTX_MISSING_TITLE",
                format!("Slide {slide_number} has no non-empty title placeholder"),
                location(&[("slide", json!(slide_number)), ("part", json!(format!("/{part}")))]),
                format!(
                    "ooxml --json pptx add-textbox {} --slide {slide_number} --text Title --x 457200 --y 274320 --cx 8229600 --cy 640080 --out {}",
                    command_arg(file), command_arg(&out)
                ),
                None,
            ));
        } else if let Some(title) = title
            && let Some(bounds) = bounds(title)
        {
            if let Some((reference_slide, reference)) = title_positions.get(layout_name) {
                if *reference != bounds {
                    let shape_id = title["shapeId"].as_u64().unwrap_or_default();
                    findings.push(finding(
                        "PPTX_INCONSISTENT_TITLE_POSITION",
                        format!("Slide {slide_number} title position differs from slide {reference_slide} on the same layout"),
                        shape_location(slide_number, part, shape_id as u32),
                        format!(
                            "ooxml --json pptx shapes set-bounds {} --slide {slide_number} --target shape:{shape_id} --bounds {},{},{},{} --out {}",
                            command_arg(file), reference[0], reference[1], reference[2], reference[3], command_arg(&out)
                        ),
                        Some(json!({"referenceSlide": reference_slide, "expectedBounds": reference, "actualBounds": bounds})),
                    ));
                }
            } else {
                title_positions.insert(layout_name.to_string(), (slide_number, bounds));
            }
        }

        if let Some(report) = layout_slides.and_then(|reports| reports.get(index)) {
            for violation in report["safeMarginViolations"]
                .as_array()
                .into_iter()
                .flatten()
            {
                let shape_id = violation["shapeId"].as_u64().unwrap_or_default();
                findings.push(finding(
                    "PPTX_OUTSIDE_SAFE_MARGIN",
                    format!("Slide {slide_number} shape {shape_id} enters the safe margin"),
                    shape_location(slide_number, part, shape_id as u32),
                    violation["fixCommand"].as_str().unwrap_or_default(),
                    Some(violation.clone()),
                ));
            }
        }
    }
    Ok(findings)
}

fn shape_location(slide: u32, part: &str, shape_id: u32) -> Value {
    location(&[
        ("slide", json!(slide)),
        ("part", json!(format!("/{part}"))),
        ("shapeId", json!(shape_id)),
    ])
}

fn shape_text(shape: &Value) -> String {
    shape["paragraphs"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|paragraph| paragraph["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

fn bounds(shape: &Value) -> Option<[i64; 4]> {
    let bounds = shape.get("bounds")?;
    Some([
        bounds["x"].as_i64()?,
        bounds["y"].as_i64()?,
        bounds["cx"].as_i64()?,
        bounds["cy"].as_i64()?,
    ])
}

fn scan_slide_styles(xml: &str) -> CliResult<BTreeMap<u32, ShapeStyle>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut styles = BTreeMap::<u32, ShapeStyle>::new();
    let mut current_id = 0_u32;
    let mut in_text = false;
    let mut text_properties_depth = 0_usize;
    let mut depth = 0_usize;
    loop {
        let event = reader.read_event();
        let event_depth = depth + 1;
        match event {
            Ok(Event::Start(element)) => {
                let name = local_name(element.name().as_ref()).to_string();
                scan_style_element(
                    &element,
                    &name,
                    event_depth,
                    &mut current_id,
                    &mut styles,
                    &mut text_properties_depth,
                );
                if name == "t" {
                    in_text = true;
                }
                depth += 1;
            }
            Ok(Event::Empty(element)) => {
                let name = local_name(element.name().as_ref()).to_string();
                scan_style_element(
                    &element,
                    &name,
                    event_depth,
                    &mut current_id,
                    &mut styles,
                    &mut text_properties_depth,
                );
            }
            Ok(event) if in_text && is_xml_text_event(&event) => {
                if let Some(style) = styles.get_mut(&current_id) {
                    append_xml_text_event(&mut style.text, &event);
                }
            }
            Ok(Event::End(element)) => {
                let name = local_name(element.name().as_ref()).to_string();
                if name == "t" {
                    in_text = false;
                }
                if matches!(name.as_str(), "rPr" | "defRPr" | "endParaRPr")
                    && text_properties_depth == depth
                {
                    text_properties_depth = 0;
                }
                depth = depth.saturating_sub(1);
            }
            Ok(Event::Eof) => break,
            Err(error) => {
                return Err(CliError::unexpected(format!(
                    "failed to parse PPTX slide styling: {error}"
                )));
            }
            _ => {}
        }
    }
    Ok(styles)
}

fn scan_style_element(
    element: &quick_xml::events::BytesStart<'_>,
    name: &str,
    depth: usize,
    current_id: &mut u32,
    styles: &mut BTreeMap<u32, ShapeStyle>,
    text_properties_depth: &mut usize,
) {
    if name == "cNvPr" {
        *current_id = attr(element, "id")
            .and_then(|value| value.parse().ok())
            .unwrap_or(0);
        let style = styles.entry(*current_id).or_default();
        style.id = *current_id;
        style.alt_text = attr(element, "descr")
            .filter(|value| !value.trim().is_empty())
            .or_else(|| attr(element, "title"))
            .unwrap_or_default();
    }
    if matches!(name, "rPr" | "defRPr" | "endParaRPr") {
        *text_properties_depth = depth;
        if let Some(size) = attr(element, "sz").and_then(|value| value.parse::<f64>().ok())
            && let Some(style) = styles.get_mut(current_id)
        {
            style.font_sizes.push(size / 100.0);
        }
    } else if *text_properties_depth > 0 && depth > *text_properties_depth {
        if name == "latin"
            && let Some(font) = attr(element, "typeface").filter(|font| !font.trim().is_empty())
            && let Some(style) = styles.get_mut(current_id)
        {
            style.fonts.insert(font);
        } else if matches!(name, "srgbClr" | "schemeClr")
            && let Some(color) = attr(element, "val")
            && let Some(style) = styles.get_mut(current_id)
        {
            style.colors.insert(color);
        }
    }
}

fn scan_theme(file: &str, entries: &[String]) -> CliResult<ThemePresentation> {
    let Some(part) = entries
        .iter()
        .find(|part| part.starts_with("ppt/theme/") && part.ends_with(".xml"))
    else {
        return Ok(ThemePresentation::default());
    };
    let xml = zip_text(file, part)?;
    let mut reader = Reader::from_str(&xml);
    let mut theme = ThemePresentation::default();
    let mut color_key = String::new();
    let mut in_font_scheme = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) => {
                let name = local_name(element.name().as_ref()).to_string();
                if matches!(
                    name.as_str(),
                    "dk1"
                        | "lt1"
                        | "dk2"
                        | "lt2"
                        | "accent1"
                        | "accent2"
                        | "accent3"
                        | "accent4"
                        | "accent5"
                        | "accent6"
                        | "hlink"
                        | "folHlink"
                ) {
                    color_key = name.to_string();
                } else if name == "fontScheme" {
                    in_font_scheme = true;
                } else {
                    scan_theme_value(&element, &name, &color_key, in_font_scheme, &mut theme);
                }
            }
            Ok(Event::Empty(element)) => scan_theme_value(
                &element,
                local_name(element.name().as_ref()),
                &color_key,
                in_font_scheme,
                &mut theme,
            ),
            Ok(Event::End(element)) => {
                let name = local_name(element.name().as_ref()).to_string();
                if name == color_key {
                    color_key.clear();
                }
                if name == "fontScheme" {
                    in_font_scheme = false;
                }
            }
            Ok(Event::Eof) => break,
            Err(error) => {
                return Err(CliError::unexpected(format!(
                    "failed to parse PPTX theme: {error}"
                )));
            }
            _ => {}
        }
    }
    Ok(theme)
}

fn scan_theme_value(
    element: &quick_xml::events::BytesStart<'_>,
    name: &str,
    color_key: &str,
    in_font_scheme: bool,
    theme: &mut ThemePresentation,
) {
    if !color_key.is_empty() && matches!(name, "srgbClr" | "sysClr") {
        let value = attr(element, if name == "sysClr" { "lastClr" } else { "val" });
        if let Some(value) = value {
            theme.colors.insert(color_key.to_string(), value);
        }
    }
    if in_font_scheme
        && name == "latin"
        && let Some(font) = attr(element, "typeface").filter(|font| !font.trim().is_empty())
    {
        theme.fonts.insert(font);
    }
}

fn resolved_background(
    slide: &str,
    layout: Option<&str>,
    theme: &BTreeMap<String, String>,
) -> Srgb {
    background_color(slide, theme)
        .or_else(|| layout.and_then(|xml| background_color(xml, theme)))
        .unwrap_or(Srgb::WHITE)
}

fn background_color(xml: &str, theme: &BTreeMap<String, String>) -> Option<Srgb> {
    let bg_start = xml.find("<p:bg")?;
    let bg_end = xml[bg_start..]
        .find("</p:bg>")
        .map(|offset| bg_start + offset)
        .unwrap_or(xml.len());
    let fragment = &xml[bg_start..bg_end];
    for marker in ["srgbClr", "schemeClr"] {
        let Some(start) = fragment.find(marker) else {
            continue;
        };
        let Some(offset) = fragment[start..].find("val=\"") else {
            continue;
        };
        let value_start = offset + start + 5;
        let Some(offset) = fragment[value_start..].find('"') else {
            continue;
        };
        let value_end = offset + value_start;
        let value = &fragment[value_start..value_end];
        if let Some(color) = resolve_color(value, theme) {
            return Some(color);
        }
    }
    None
}

fn resolve_color(value: &str, theme: &BTreeMap<String, String>) -> Option<Srgb> {
    let resolved = theme.get(value).map(String::as_str).unwrap_or(value);
    Srgb::from_hex(resolved).ok()
}

fn is_theme_font(font: &str, theme_fonts: &BTreeSet<String>) -> bool {
    matches!(
        font,
        "+mj-lt" | "+mn-lt" | "+mj-ea" | "+mn-ea" | "+mj-cs" | "+mn-cs"
    ) || theme_fonts.contains(font)
}

fn image_scale_evidence(file: &str, shape: &Value) -> CliResult<Option<Value>> {
    let Some(displayed) = bounds(shape) else {
        return Ok(None);
    };
    let target = shape["imageRef"]["targetUri"]
        .as_str()
        .unwrap_or_default()
        .trim_start_matches('/');
    if target.is_empty() {
        return Ok(None);
    }
    let bytes = zip_bytes(file, target)?;
    let Some((pixels_w, pixels_h)) = image_pixel_size(&bytes) else {
        return Ok(None);
    };
    let native_w = i64::from(pixels_w) * EMU_PER_PIXEL_96_DPI;
    let native_h = i64::from(pixels_h) * EMU_PER_PIXEL_96_DPI;
    if native_w == 0 || native_h == 0 || displayed[2] == 0 || displayed[3] == 0 {
        return Ok(None);
    }
    let displayed_ratio = displayed[2] as f64 / displayed[3] as f64;
    let native_ratio = native_w as f64 / native_h as f64;
    let distortion = (displayed_ratio / native_ratio - 1.0).abs();
    let upscale = displayed[2] > native_w || displayed[3] > native_h;
    Ok((upscale || distortion > 0.03).then(|| {
        json!({
            "displayedEmu": {"width": displayed[2], "height": displayed[3]},
            "nativePixels": {"width": pixels_w, "height": pixels_h},
            "assumedDpi": 96,
            "upscaled": upscale,
            "aspectDistortion": distortion,
        })
    }))
}

fn image_pixel_size(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") && bytes.len() >= 24 {
        return Some((
            u32::from_be_bytes(bytes[16..20].try_into().ok()?),
            u32::from_be_bytes(bytes[20..24].try_into().ok()?),
        ));
    }
    if bytes.starts_with(&[0xff, 0xd8]) {
        let mut offset = 2;
        while offset + 9 < bytes.len() {
            if bytes[offset] != 0xff {
                offset += 1;
                continue;
            }
            let marker = bytes[offset + 1];
            if matches!(marker, 0xc0..=0xc3 | 0xc5..=0xc7 | 0xc9..=0xcb | 0xcd..=0xcf) {
                return Some((
                    u16::from_be_bytes([bytes[offset + 7], bytes[offset + 8]]) as u32,
                    u16::from_be_bytes([bytes[offset + 5], bytes[offset + 6]]) as u32,
                ));
            }
            let length = u16::from_be_bytes([bytes[offset + 2], bytes[offset + 3]]) as usize;
            if length < 2 {
                break;
            }
            offset += length + 2;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slide_style_scanner_collects_font_size_color_font_and_alt() {
        let styles = scan_slide_styles(r#"<p:sld xmlns:p="p" xmlns:a="a"><p:cSld><p:sp><p:nvSpPr><p:cNvPr id="7" name="Text" descr="Accessible"/></p:nvSpPr><p:txBody><a:p><a:r><a:rPr sz="1100"><a:latin typeface="Arial"/><a:solidFill><a:srgbClr val="EEEEEE"/></a:solidFill></a:rPr><a:t>Hello</a:t></a:r></a:p></p:txBody></p:sp></p:cSld></p:sld>"#).unwrap();
        assert_eq!(styles[&7].font_sizes, [11.0]);
        assert!(styles[&7].fonts.contains("Arial"));
        assert!(styles[&7].colors.contains("EEEEEE"));
        assert_eq!(styles[&7].alt_text, "Accessible");
    }

    #[test]
    fn image_dimension_reader_supports_png() {
        let mut bytes = b"\x89PNG\r\n\x1a\n00000000".to_vec();
        bytes.extend_from_slice(&640_u32.to_be_bytes());
        bytes.extend_from_slice(&480_u32.to_be_bytes());
        assert_eq!(image_pixel_size(&bytes), Some((640, 480)));
    }

    #[test]
    fn contrast_evidence_uses_palette_oklch_and_wcag_ratio() {
        let foreground = Srgb::from_hex("EEEEEE").unwrap();
        let background = Srgb::WHITE;
        assert!(foreground.contrast_ratio(background) < 4.5);
        assert!(foreground.to_oklch().l > 0.9);
    }
}
