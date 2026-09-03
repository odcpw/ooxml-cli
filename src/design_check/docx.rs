use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

use super::{DesignConfig, DesignFinding, finding, fixed_output_path, location};
use crate::{
    CliError, CliResult, append_xml_text_event, attr, command_arg, docx_rich_block_reports,
    find_docx_document_part, is_xml_text_event, local_name, zip_text,
};

const DEFAULT_PAGE_WIDTH_TWIPS: i64 = 12_240;
const DEFAULT_MARGIN_TWIPS: i64 = 1_440;
const EMU_PER_TWIP: i64 = 635;

#[derive(Default)]
struct Paragraph {
    block: usize,
    text: String,
    style: String,
    direct_properties: BTreeSet<String>,
    fonts: BTreeSet<String>,
}

#[derive(Default)]
struct Table {
    block: usize,
    index: usize,
    width_twips: i64,
    grid_width_twips: i64,
}

#[derive(Default)]
struct Image {
    block: usize,
    index: usize,
    width_emu: i64,
    alt_text: String,
}

struct DocumentScan {
    paragraphs: Vec<Paragraph>,
    tables: Vec<Table>,
    images: Vec<Image>,
    text_width_twips: i64,
}

#[derive(Default)]
struct StyleDefinition {
    properties: BTreeSet<String>,
}

pub(super) fn analyze(
    file: &str,
    entries: &[String],
    config: &DesignConfig,
) -> CliResult<Vec<DesignFinding>> {
    let document_part = find_docx_document_part(file, entries)?;
    let document_xml = zip_text(file, &document_part)?;
    let scan = scan_document(&document_xml)?;
    let styles_xml = entries
        .iter()
        .find(|entry| entry.ends_with("/styles.xml"))
        .map(|part| zip_text(file, part))
        .transpose()?
        .unwrap_or_default();
    let styles = scan_styles(&styles_xml)?;
    let theme_fonts = scan_theme_fonts(file, entries)?;
    let blocks = docx_rich_block_reports(&document_xml, false)?;
    let block_hashes = blocks
        .iter()
        .map(|block| (block.index, block.content_hash.as_str()))
        .collect::<BTreeMap<_, _>>();
    let mut findings = Vec::new();

    for diagnostic in crate::docx_styles::validate_docx_style_integrity(file, entries)? {
        if !matches!(
            diagnostic["code"].as_str(),
            Some("DOCX_DANGLING_STYLE" | "DOCX_DANGLING_NUMBERING")
        ) {
            continue;
        }
        let style_id = diagnostic["styleId"].as_str().unwrap_or_default();
        let element = diagnostic["element"].as_str().unwrap_or("style");
        let paragraph = scan
            .paragraphs
            .iter()
            .find(|paragraph| paragraph.style == style_id);
        let target = match element {
            "tblStyle" => "table",
            "rStyle" => "run",
            _ => "paragraph",
        };
        let index = paragraph.map(|paragraph| paragraph.block).unwrap_or(1);
        findings.push(finding(
            "DOCX_DANGLING_STYLE",
            diagnostic["message"]
                .as_str()
                .unwrap_or("DOCX content has a dangling style reference"),
            location(&[
                ("part", diagnostic["part"].clone()),
                ("element", json!(element)),
                ("block", json!(index)),
            ]),
            format!(
                "ooxml --json docx styles apply {} --target {target} --index {index} --style Normal --out {}",
                command_arg(file),
                command_arg(&fixed_output_path(file, "style-fixed"))
            ),
            Some(json!({"styleId": style_id})),
        ));
    }

    let mut previous_heading = None::<u32>;
    for paragraph in &scan.paragraphs {
        let Some(level) = heading_level(&paragraph.style) else {
            continue;
        };
        if let Some(previous) = previous_heading
            && level > previous + 1
        {
            let replacement = previous + 1;
            findings.push(finding(
                "DOCX_HEADING_LEVEL_SKIP",
                format!(
                    "Heading level {level} follows heading level {previous}; use Heading {replacement}"
                ),
                location(&[
                    ("part", json!(format!("/{document_part}"))),
                    ("block", json!(paragraph.block)),
                ]),
                format!(
                    "ooxml --json docx styles apply {} --target paragraph --index {} --style Heading{} --out {}",
                    command_arg(file),
                    paragraph.block,
                    replacement,
                    command_arg(&fixed_output_path(file, "heading-fixed"))
                ),
                Some(json!({"previousLevel": previous, "level": level})),
            ));
        }
        previous_heading = Some(level);
    }

    let allowed_empty = config.threshold("docx.maxConsecutiveEmptyParagraphs", 3.0) as usize;
    let mut run_start = 0_usize;
    let mut run_length = 0_usize;
    let sentinel = Paragraph {
        text: "sentinel".to_string(),
        ..Paragraph::default()
    };
    for paragraph in scan.paragraphs.iter().chain(std::iter::once(&sentinel)) {
        if paragraph.text.trim().is_empty() {
            if run_length == 0 {
                run_start = paragraph.block;
            }
            run_length += 1;
        } else {
            if run_length > allowed_empty {
                let first_excess = run_start + allowed_empty;
                findings.push(finding(
                    "DOCX_EXCESS_EMPTY_PARAGRAPHS",
                    format!(
                        "{run_length} consecutive empty paragraphs exceed the configured maximum of {allowed_empty}"
                    ),
                    location(&[
                        ("part", json!(format!("/{document_part}"))),
                        ("block", json!(first_excess)),
                    ]),
                    format!(
                        "ooxml --json docx blocks delete {} --block {first_excess} --out {}",
                        command_arg(file),
                        command_arg(&fixed_output_path(file, "spacing-fixed"))
                    ),
                    Some(json!({"startBlock": run_start, "count": run_length})),
                ));
            }
            run_length = 0;
        }
    }

    for table in &scan.tables {
        if table.width_twips > scan.text_width_twips {
            findings.push(finding(
                "DOCX_TABLE_TOO_WIDE",
                format!(
                    "Table {} is {} twips wide but the section text area is {} twips",
                    table.index, table.width_twips, scan.text_width_twips
                ),
                location(&[
                    ("part", json!(format!("/{document_part}"))),
                    ("block", json!(table.block)),
                    ("table", json!(table.index)),
                ]),
                format!(
                    "ooxml --json docx sections set {} --section 1 --orientation landscape --out {}",
                    command_arg(file),
                    command_arg(&fixed_output_path(file, "table-width-fixed"))
                ),
                Some(json!({
                    "tableWidthTwips": table.width_twips,
                    "textWidthTwips": scan.text_width_twips,
                })),
            ));
        }
    }

    for image in &scan.images {
        if image.width_emu > scan.text_width_twips * EMU_PER_TWIP {
            findings.push(finding(
                "DOCX_IMAGE_TOO_WIDE",
                format!(
                    "Image {} is wider than the section text area",
                    image.index
                ),
                location(&[
                    ("part", json!(format!("/{document_part}"))),
                    ("block", json!(image.block)),
                    ("image", json!(image.index)),
                ]),
                format!(
                    "ooxml --json docx sections set {} --section 1 --orientation landscape --out {}",
                    command_arg(file),
                    command_arg(&fixed_output_path(file, "image-width-fixed"))
                ),
                Some(json!({
                    "imageWidthEmu": image.width_emu,
                    "textWidthEmu": scan.text_width_twips * EMU_PER_TWIP,
                })),
            ));
        }
        if image.alt_text.trim().is_empty() {
            findings.push(finding(
                "DOCX_MISSING_ALT_TEXT",
                format!("Image {} has no title or description", image.index),
                location(&[
                    ("part", json!(format!("/{document_part}"))),
                    ("block", json!(image.block)),
                    ("image", json!(image.index)),
                ]),
                format!(
                    "ooxml --json docx blocks delete {} --block {} --out {}",
                    command_arg(file),
                    image.block,
                    command_arg(&fixed_output_path(file, "accessibility-fixed"))
                ),
                None,
            ));
        }
    }

    for paragraph in &scan.paragraphs {
        let Some(style) = styles.get(&paragraph.style) else {
            continue;
        };
        if !paragraph.direct_properties.is_empty()
            && paragraph
                .direct_properties
                .iter()
                .all(|property| style.properties.contains(property))
        {
            findings.push(finding(
                "DOCX_REDUNDANT_DIRECT_FORMATTING",
                format!(
                    "Block {} repeats formatting already supplied by style {}",
                    paragraph.block, paragraph.style
                ),
                location(&[
                    ("part", json!(format!("/{document_part}"))),
                    ("block", json!(paragraph.block)),
                ]),
                replace_block_command(
                    file,
                    paragraph,
                    block_hashes
                        .get(&paragraph.block)
                        .copied()
                        .unwrap_or_default(),
                    "direct-format-fixed",
                ),
                Some(json!({"styleId": paragraph.style})),
            ));
        }
        let outside = paragraph
            .fonts
            .iter()
            .filter(|font| !theme_fonts.contains(*font))
            .cloned()
            .collect::<Vec<_>>();
        if !outside.is_empty() {
            findings.push(finding(
                "DOCX_FONT_OUTSIDE_THEME",
                format!(
                    "Block {} uses font(s) outside the theme: {}",
                    paragraph.block,
                    outside.join(", ")
                ),
                location(&[
                    ("part", json!(format!("/{document_part}"))),
                    ("block", json!(paragraph.block)),
                ]),
                replace_block_command(
                    file,
                    paragraph,
                    block_hashes
                        .get(&paragraph.block)
                        .copied()
                        .unwrap_or_default(),
                    "font-fixed",
                ),
                Some(json!({"fonts": outside, "themeFonts": &theme_fonts})),
            ));
        }
    }

    Ok(findings)
}

fn replace_block_command(file: &str, paragraph: &Paragraph, hash: &str, suffix: &str) -> String {
    let style = if paragraph.style.is_empty() {
        "Normal"
    } else {
        &paragraph.style
    };
    let hash_flag = if hash.is_empty() {
        String::new()
    } else {
        format!(" --expect-hash {}", command_arg(hash))
    };
    format!(
        "ooxml --json docx blocks replace {} --block {}{} --text {} --style {} --out {}",
        command_arg(file),
        paragraph.block,
        hash_flag,
        command_arg(&paragraph.text),
        command_arg(style),
        command_arg(&fixed_output_path(file, suffix))
    )
}

fn scan_document(xml: &str) -> CliResult<DocumentScan> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut stack = Vec::<String>::new();
    let mut block = 0_usize;
    let mut paragraph = None::<Paragraph>;
    let mut paragraph_depth = 0_usize;
    let mut table = None::<Table>;
    let mut table_depth = 0_usize;
    let mut image = None::<Image>;
    let mut drawing_depth = 0_usize;
    let mut in_text = false;
    let mut run_properties_depth = 0_usize;
    let mut paragraphs = Vec::new();
    let mut tables = Vec::new();
    let mut images = Vec::new();
    let mut page_width = DEFAULT_PAGE_WIDTH_TWIPS;
    let mut left_margin = DEFAULT_MARGIN_TWIPS;
    let mut right_margin = DEFAULT_MARGIN_TWIPS;

    loop {
        let event = reader.read_event();
        match event {
            Ok(Event::Start(element)) => {
                let name = local_name(element.name().as_ref()).to_string();
                let depth = stack.len() + 1;
                if stack.last().is_some_and(|parent| parent == "body") && name == "p" {
                    block += 1;
                    paragraph = Some(Paragraph {
                        block,
                        ..Paragraph::default()
                    });
                    paragraph_depth = depth;
                } else if stack.last().is_some_and(|parent| parent == "body") && name == "tbl" {
                    block += 1;
                    table = Some(Table {
                        block,
                        index: tables.len() + 1,
                        ..Table::default()
                    });
                    table_depth = depth;
                }
                let mut state = DocumentElementState {
                    paragraph: &mut paragraph,
                    table: &mut table,
                    image: &mut image,
                    drawing_depth: &mut drawing_depth,
                    run_properties_depth: &mut run_properties_depth,
                    page_width: &mut page_width,
                    left_margin: &mut left_margin,
                    right_margin: &mut right_margin,
                };
                scan_document_element(&element, &name, depth, &mut state, block, images.len() + 1);
                if name == "t" {
                    in_text = true;
                }
                stack.push(name);
            }
            Ok(Event::Empty(element)) => {
                let name = local_name(element.name().as_ref()).to_string();
                let depth = stack.len() + 1;
                if stack.last().is_some_and(|parent| parent == "body") && name == "p" {
                    block += 1;
                    paragraphs.push(Paragraph {
                        block,
                        ..Paragraph::default()
                    });
                    continue;
                }
                if stack.last().is_some_and(|parent| parent == "body") && name == "tbl" {
                    block += 1;
                    tables.push(Table {
                        block,
                        index: tables.len() + 1,
                        ..Table::default()
                    });
                    continue;
                }
                let mut state = DocumentElementState {
                    paragraph: &mut paragraph,
                    table: &mut table,
                    image: &mut image,
                    drawing_depth: &mut drawing_depth,
                    run_properties_depth: &mut run_properties_depth,
                    page_width: &mut page_width,
                    left_margin: &mut left_margin,
                    right_margin: &mut right_margin,
                };
                scan_document_element(&element, &name, depth, &mut state, block, images.len() + 1);
            }
            Ok(event) if in_text && is_xml_text_event(&event) => {
                if let Some(paragraph) = paragraph.as_mut() {
                    append_xml_text_event(&mut paragraph.text, &event);
                }
            }
            Ok(Event::End(element)) => {
                let name = local_name(element.name().as_ref()).to_string();
                let depth = stack.len();
                if name == "t" {
                    in_text = false;
                }
                if run_properties_depth == depth && name == "rPr" {
                    run_properties_depth = 0;
                }
                if drawing_depth == depth && name == "drawing" {
                    if let Some(image) = image.take() {
                        images.push(image);
                    }
                    drawing_depth = 0;
                }
                if paragraph_depth == depth && name == "p" {
                    if let Some(paragraph) = paragraph.take() {
                        paragraphs.push(paragraph);
                    }
                    paragraph_depth = 0;
                }
                if table_depth == depth && name == "tbl" {
                    if let Some(mut table) = table.take() {
                        if table.width_twips == 0 {
                            table.width_twips = table.grid_width_twips;
                        }
                        tables.push(table);
                    }
                    table_depth = 0;
                }
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Err(error) => {
                return Err(CliError::unexpected(format!(
                    "failed to parse DOCX document for design checks: {error}"
                )));
            }
            _ => {}
        }
    }
    Ok(DocumentScan {
        paragraphs,
        tables,
        images,
        text_width_twips: (page_width - left_margin - right_margin).max(1),
    })
}

struct DocumentElementState<'a> {
    paragraph: &'a mut Option<Paragraph>,
    table: &'a mut Option<Table>,
    image: &'a mut Option<Image>,
    drawing_depth: &'a mut usize,
    run_properties_depth: &'a mut usize,
    page_width: &'a mut i64,
    left_margin: &'a mut i64,
    right_margin: &'a mut i64,
}

fn scan_document_element(
    element: &quick_xml::events::BytesStart<'_>,
    name: &str,
    depth: usize,
    state: &mut DocumentElementState<'_>,
    block: usize,
    image_index: usize,
) {
    if name == "rPr" && state.paragraph.is_some() {
        *state.run_properties_depth = depth;
    }
    if let Some(paragraph) = state.paragraph.as_mut() {
        if name == "pStyle" {
            paragraph.style = attr(element, "val").unwrap_or_default();
        }
        if *state.run_properties_depth > 0 && depth > *state.run_properties_depth {
            if let Some(property) = formatting_property(element, name) {
                paragraph.direct_properties.insert(property);
            }
            if name == "rFonts" {
                for attribute in ["ascii", "hAnsi", "eastAsia", "cs"] {
                    if let Some(font) = attr(element, attribute).filter(|font| !font.is_empty()) {
                        paragraph.fonts.insert(font);
                    }
                }
            }
        }
    }
    if let Some(table) = state.table.as_mut() {
        if name == "tblW"
            && attr(element, "type").as_deref() == Some("dxa")
            && let Some(width) = attr(element, "w").and_then(|value| value.parse().ok())
        {
            table.width_twips = width;
        } else if name == "gridCol"
            && let Some(width) = attr(element, "w").and_then(|value| value.parse::<i64>().ok())
        {
            table.grid_width_twips += width;
        }
    }
    if name == "drawing" {
        *state.drawing_depth = depth;
        *state.image = Some(Image {
            block,
            index: image_index,
            ..Image::default()
        });
    } else if *state.drawing_depth > 0 {
        if name == "extent"
            && let Some(width) = attr(element, "cx").and_then(|value| value.parse().ok())
            && let Some(image) = state.image.as_mut()
        {
            image.width_emu = width;
        } else if name == "docPr"
            && let Some(image) = state.image.as_mut()
        {
            image.alt_text = attr(element, "descr")
                .filter(|value| !value.trim().is_empty())
                .or_else(|| attr(element, "title"))
                .unwrap_or_default();
        }
    }
    if name == "pgSz"
        && let Some(width) = attr(element, "w").and_then(|value| value.parse().ok())
    {
        *state.page_width = width;
    } else if name == "pgMar" {
        if let Some(value) = attr(element, "left").and_then(|value| value.parse().ok()) {
            *state.left_margin = value;
        }
        if let Some(value) = attr(element, "right").and_then(|value| value.parse().ok()) {
            *state.right_margin = value;
        }
    }
}

fn scan_styles(xml: &str) -> CliResult<BTreeMap<String, StyleDefinition>> {
    let mut reader = Reader::from_str(xml);
    let mut current = None::<(String, StyleDefinition)>;
    let mut run_properties_depth = 0_usize;
    let mut depth = 0_usize;
    let mut styles = BTreeMap::new();
    loop {
        let event = reader.read_event();
        let event_depth = depth + 1;
        match event {
            Ok(Event::Start(element)) => {
                let name = local_name(element.name().as_ref()).to_string();
                if name == "style" {
                    current = Some((
                        attr(&element, "styleId").unwrap_or_default(),
                        StyleDefinition::default(),
                    ));
                } else if name == "rPr" && current.is_some() {
                    run_properties_depth = event_depth;
                } else if run_properties_depth > 0
                    && event_depth > run_properties_depth
                    && let Some(property) = formatting_property(&element, &name)
                    && let Some((_, style)) = current.as_mut()
                {
                    style.properties.insert(property);
                }
                depth += 1;
            }
            Ok(Event::Empty(element)) => {
                let name = local_name(element.name().as_ref()).to_string();
                if run_properties_depth > 0
                    && event_depth > run_properties_depth
                    && let Some(property) = formatting_property(&element, &name)
                    && let Some((_, style)) = current.as_mut()
                {
                    style.properties.insert(property);
                }
            }
            Ok(Event::End(element)) => {
                let name = local_name(element.name().as_ref()).to_string();
                if name == "rPr" && run_properties_depth == depth {
                    run_properties_depth = 0;
                }
                if name == "style"
                    && let Some((id, style)) = current.take()
                    && !id.is_empty()
                {
                    styles.insert(id, style);
                }
                depth = depth.saturating_sub(1);
            }
            Ok(Event::Eof) => break,
            Err(error) => {
                return Err(CliError::unexpected(format!(
                    "failed to parse DOCX styles for design checks: {error}"
                )));
            }
            _ => {}
        }
    }
    Ok(styles)
}

fn scan_theme_fonts(file: &str, entries: &[String]) -> CliResult<BTreeSet<String>> {
    let Some(part) = entries
        .iter()
        .find(|entry| entry.starts_with("word/theme/") && entry.ends_with(".xml"))
    else {
        return Ok(BTreeSet::new());
    };
    let xml = zip_text(file, part)?;
    let mut reader = Reader::from_str(&xml);
    let mut fonts = BTreeSet::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) | Ok(Event::Empty(element))
                if local_name(element.name().as_ref()) == "latin" =>
            {
                if let Some(font) = attr(&element, "typeface").filter(|font| !font.is_empty()) {
                    fonts.insert(font);
                }
            }
            Ok(Event::Eof) => break,
            Err(error) => return Err(CliError::unexpected(error.to_string())),
            _ => {}
        }
    }
    Ok(fonts)
}

fn formatting_property(element: &quick_xml::events::BytesStart<'_>, name: &str) -> Option<String> {
    match name {
        "b" | "i" | "u" | "strike" => Some(format!(
            "{name}:{}",
            attr(element, "val").unwrap_or_else(|| "true".to_string())
        )),
        "color" | "sz" | "highlight" => attr(element, "val").map(|value| format!("{name}:{value}")),
        "rFonts" => attr(element, "ascii")
            .or_else(|| attr(element, "hAnsi"))
            .map(|value| format!("font:{value}")),
        _ => None,
    }
}

fn heading_level(style: &str) -> Option<u32> {
    let normalized = style.replace(' ', "").to_ascii_lowercase();
    normalized
        .strip_prefix("heading")
        .and_then(|level| level.parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_scanner_drives_all_docx_geometry_and_paragraph_rules() {
        let xml = r#"<w:document xmlns:w="w" xmlns:wp="wp"><w:body>
          <w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>One</w:t></w:r></w:p>
          <w:p><w:pPr><w:pStyle w:val="Heading3"/></w:pPr><w:r><w:rPr><w:b/><w:rFonts w:ascii="Comic Sans MS"/></w:rPr><w:t>Three</w:t></w:r></w:p>
          <w:p/><w:p/><w:p/><w:p/>
          <w:tbl><w:tblPr><w:tblW w:type="dxa" w:w="10000"/></w:tblPr></w:tbl>
          <w:p><w:r><w:drawing><wp:extent cx="7000000" cy="1"/><wp:docPr id="1" name="Picture"/></w:drawing></w:r></w:p>
          <w:sectPr><w:pgSz w:w="12240"/><w:pgMar w:left="1440" w:right="1440"/></w:sectPr>
        </w:body></w:document>"#;
        let scan = scan_document(xml).unwrap();
        assert_eq!(scan.paragraphs.len(), 7);
        assert_eq!(heading_level(&scan.paragraphs[1].style), Some(3));
        assert_eq!(scan.tables[0].width_twips, 10_000);
        assert!(scan.tables[0].width_twips > scan.text_width_twips);
        assert!(scan.images[0].alt_text.is_empty());
        assert!(scan.images[0].width_emu > scan.text_width_twips * EMU_PER_TWIP);
        assert!(scan.paragraphs[1].fonts.contains("Comic Sans MS"));
    }

    #[test]
    fn style_scanner_detects_redundant_direct_properties() {
        let styles = scan_styles(
            r#"<w:styles xmlns:w="w"><w:style w:styleId="Quote"><w:rPr><w:i/><w:color w:val="666666"/></w:rPr></w:style></w:styles>"#,
        )
        .unwrap();
        assert!(styles["Quote"].properties.contains("i:true"));
        assert!(styles["Quote"].properties.contains("color:666666"));
    }
}
