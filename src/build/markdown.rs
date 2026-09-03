//! Deterministic Markdown-to-build-spec conversion.
//!
//! This parser intentionally implements the small, documented Markdown input
//! profile used by the build commands. It keeps source line numbers on every
//! block so syntax outside that profile is reported instead of silently lost.

use serde::Serialize;
use serde_json::{Map, Value, json};
use std::fmt;

use crate::build::{BuildFamily, load_spec_bytes};

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkdownWarning {
    pub line: usize,
    pub code: String,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkdownConversion {
    pub spec: Value,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<MarkdownWarning>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkdownError {
    pub line: Option<usize>,
    pub code: String,
    pub message: String,
}

impl fmt::Display for MarkdownError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(line) = self.line {
            write!(formatter, "line {line}: {}", self.message)
        } else {
            formatter.write_str(&self.message)
        }
    }
}

impl std::error::Error for MarkdownError {}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceLine<'a> {
    number: usize,
    text: &'a str,
}

#[derive(Clone, Debug, PartialEq)]
struct ParsedMarkdown {
    front_matter: Map<String, Value>,
    blocks: Vec<Block>,
    warnings: Vec<MarkdownWarning>,
}

#[derive(Clone, Debug, PartialEq)]
enum Block {
    Heading {
        line: usize,
        level: u8,
        content: InlineContent,
    },
    Paragraph {
        line: usize,
        content: InlineContent,
    },
    ListItem {
        line: usize,
        level: u8,
        ordered: bool,
        content: InlineContent,
    },
    Image {
        line: usize,
        alt: String,
        path: String,
        width: Option<String>,
    },
    Table {
        line: usize,
        rows: Vec<Vec<String>>,
    },
    Rule {
        line: usize,
    },
    Code {
        line: usize,
        language: String,
        text: String,
    },
    Notes {
        line: usize,
        text: String,
    },
}

impl Block {
    fn line(&self) -> usize {
        match self {
            Self::Heading { line, .. }
            | Self::Paragraph { line, .. }
            | Self::ListItem { line, .. }
            | Self::Image { line, .. }
            | Self::Table { line, .. }
            | Self::Rule { line }
            | Self::Code { line, .. }
            | Self::Notes { line, .. } => *line,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
struct InlineContent {
    text: String,
    runs: Vec<InlineRun>,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct InlineRun {
    text: String,
    bold: bool,
    italic: bool,
    code: bool,
    link: Option<String>,
}

/// Convert the supported Markdown profile into a validated family build spec.
pub fn markdown_to_spec(
    family: BuildFamily,
    source: &str,
    source_name: &str,
) -> Result<MarkdownConversion, MarkdownError> {
    let parsed = parse_markdown(source)?;
    let mut conversion = match family {
        BuildFamily::Pptx => pptx_spec(parsed, source_name)?,
        BuildFamily::Docx => docx_spec(parsed, source_name)?,
        BuildFamily::Xlsx => {
            return Err(MarkdownError {
                line: None,
                code: "MARKDOWN_FAMILY_UNSUPPORTED".to_string(),
                message: "Markdown input is supported for pptx and docx builds, not xlsx"
                    .to_string(),
            });
        }
    };
    let encoded = serde_json::to_vec(&conversion.spec).map_err(|error| MarkdownError {
        line: None,
        code: "MARKDOWN_SPEC_ENCODE_FAILED".to_string(),
        message: format!("failed to encode generated build spec: {error}"),
    })?;
    load_spec_bytes(family, &encoded).map_err(|error| MarkdownError {
        line: None,
        code: "MARKDOWN_SPEC_INVALID".to_string(),
        message: format!(
            "generated {} build spec is invalid: {error}",
            family.as_str()
        ),
    })?;
    conversion.warnings.sort_by(|left, right| {
        (&left.line, &left.code, &left.message).cmp(&(&right.line, &right.code, &right.message))
    });
    conversion.warnings.dedup();
    Ok(conversion)
}

fn parse_markdown(source: &str) -> Result<ParsedMarkdown, MarkdownError> {
    let lines = source
        .lines()
        .enumerate()
        .map(|(index, text)| SourceLine {
            number: index + 1,
            text: text.strip_suffix('\r').unwrap_or(text),
        })
        .collect::<Vec<_>>();
    let (front_matter, mut index) = parse_front_matter(&lines)?;
    let mut blocks = Vec::new();
    let mut warnings = Vec::new();

    while index < lines.len() {
        let line = &lines[index];
        let trimmed = line.text.trim();
        if trimmed.is_empty() {
            index += 1;
            continue;
        }

        if let Some(language) = trimmed.strip_prefix("```") {
            let start_line = line.number;
            index += 1;
            let mut body = Vec::new();
            let mut closed = false;
            while index < lines.len() {
                if lines[index].text.trim() == "```" {
                    closed = true;
                    index += 1;
                    break;
                }
                body.push(lines[index].text);
                index += 1;
            }
            if !closed {
                warnings.push(warning(
                    start_line,
                    "MARKDOWN_UNTERMINATED_FENCE",
                    "unterminated fenced code block was preserved through end of input",
                ));
            }
            blocks.push(Block::Code {
                line: start_line,
                language: language.trim().to_ascii_lowercase(),
                text: body.join("\n"),
            });
            continue;
        }

        if trimmed.starts_with("<!--") {
            if trimmed.to_ascii_lowercase().starts_with("<!-- notes:") {
                let start_line = line.number;
                let mut comment = line.text.to_string();
                index += 1;
                while !comment.contains("-->") && index < lines.len() {
                    comment.push('\n');
                    comment.push_str(lines[index].text);
                    index += 1;
                }
                let text = comment
                    .trim()
                    .strip_prefix("<!--")
                    .unwrap_or(&comment)
                    .strip_suffix("-->")
                    .unwrap_or(&comment)
                    .trim();
                let text = text
                    .split_once(':')
                    .map(|(_, value)| value.trim())
                    .unwrap_or(text);
                blocks.push(Block::Notes {
                    line: start_line,
                    text: text.to_string(),
                });
            } else {
                warnings.push(warning(
                    line.number,
                    "MARKDOWN_HTML_UNSUPPORTED",
                    "HTML comment is outside the supported notes syntax and was preserved as text",
                ));
                blocks.push(Block::Paragraph {
                    line: line.number,
                    content: parse_inlines(line.text),
                });
                index += 1;
            }
            continue;
        }

        if trimmed.eq_ignore_ascii_case("notes:") {
            let start_line = line.number;
            index += 1;
            let mut notes = Vec::new();
            while index < lines.len() && !is_slide_boundary(lines[index].text.trim()) {
                if !lines[index].text.trim().is_empty() {
                    notes.push(lines[index].text.trim());
                }
                index += 1;
            }
            blocks.push(Block::Notes {
                line: start_line,
                text: notes.join("\n"),
            });
            continue;
        }

        if let Some((level, text)) = heading(trimmed) {
            blocks.push(Block::Heading {
                line: line.number,
                level,
                content: parse_inlines(text),
            });
            index += 1;
            continue;
        }

        if is_rule(trimmed) {
            blocks.push(Block::Rule { line: line.number });
            index += 1;
            continue;
        }

        if let Some((level, ordered, text)) = list_item(line.text) {
            blocks.push(Block::ListItem {
                line: line.number,
                level,
                ordered,
                content: parse_inlines(text),
            });
            index += 1;
            continue;
        }

        if let Some((alt, path, width)) = standalone_image(trimmed) {
            blocks.push(Block::Image {
                line: line.number,
                alt,
                path,
                width,
            });
            index += 1;
            continue;
        }

        if index + 1 < lines.len()
            && looks_like_table_row(trimmed)
            && is_table_separator(lines[index + 1].text.trim())
        {
            let start_line = line.number;
            let mut rows = vec![table_cells(trimmed)];
            index += 2;
            while index < lines.len() && looks_like_table_row(lines[index].text.trim()) {
                rows.push(table_cells(lines[index].text.trim()));
                index += 1;
            }
            let width = rows.first().map_or(0, Vec::len);
            if width == 0 || rows.iter().any(|row| row.len() != width) {
                return Err(MarkdownError {
                    line: Some(start_line),
                    code: "MARKDOWN_TABLE_RAGGED".to_string(),
                    message: "GFM table rows must have the same number of cells".to_string(),
                });
            }
            blocks.push(Block::Table {
                line: start_line,
                rows,
            });
            continue;
        }

        let start_line = line.number;
        let mut paragraph = Vec::new();
        while index < lines.len() {
            let candidate = lines[index].text.trim();
            if candidate.is_empty() || (!paragraph.is_empty() && is_block_start(&lines, index)) {
                break;
            }
            paragraph.push(candidate);
            index += 1;
        }
        let text = paragraph.join(" ");
        if text.starts_with('>') {
            warnings.push(warning(
                start_line,
                "MARKDOWN_BLOCKQUOTE_UNSUPPORTED",
                "blockquote syntax is not styled by the build profile; its text was preserved",
            ));
        }
        blocks.push(Block::Paragraph {
            line: start_line,
            content: parse_inlines(&text),
        });
    }

    Ok(ParsedMarkdown {
        front_matter,
        blocks,
        warnings,
    })
}

fn parse_front_matter(
    lines: &[SourceLine<'_>],
) -> Result<(Map<String, Value>, usize), MarkdownError> {
    if lines.first().map(|line| line.text.trim()) != Some("---") {
        return Ok((Map::new(), 0));
    }
    let Some(end) = lines
        .iter()
        .enumerate()
        .skip(1)
        .find_map(|(index, line)| (line.text.trim() == "---").then_some(index))
    else {
        return Ok((Map::new(), 0));
    };
    let source = lines[1..end]
        .iter()
        .map(|line| line.text)
        .collect::<Vec<_>>()
        .join("\n");
    let trimmed = source.trim();
    if trimmed.is_empty() {
        return Ok((Map::new(), end + 1));
    }
    if trimmed.starts_with('{') {
        let value: Value = serde_json::from_str(trimmed).map_err(|error| MarkdownError {
            line: Some(2 + error.line().saturating_sub(1)),
            code: "MARKDOWN_FRONT_MATTER_INVALID".to_string(),
            message: format!("invalid JSON front matter: {error}"),
        })?;
        return value
            .as_object()
            .cloned()
            .map(|object| (object, end + 1))
            .ok_or_else(|| MarkdownError {
                line: Some(2),
                code: "MARKDOWN_FRONT_MATTER_INVALID".to_string(),
                message: "JSON front matter must be an object".to_string(),
            });
    }

    let mut object = Map::new();
    let mut object_path = Vec::<(usize, String)>::new();
    for line in &lines[1..end] {
        if line.text.trim().is_empty() || line.text.trim_start().starts_with('#') {
            continue;
        }
        if line.text.starts_with('\t') {
            return Err(MarkdownError {
                line: Some(line.number),
                code: "MARKDOWN_FRONT_MATTER_INVALID".to_string(),
                message: "YAML front matter indentation must use spaces".to_string(),
            });
        }
        let indent = line.text.len() - line.text.trim_start().len();
        let Some((raw_key, raw_value)) = line.text.trim().split_once(':') else {
            return Err(MarkdownError {
                line: Some(line.number),
                code: "MARKDOWN_FRONT_MATTER_INVALID".to_string(),
                message: "YAML front matter entries must use key: value".to_string(),
            });
        };
        let key = raw_key.trim();
        if key.is_empty() {
            return Err(MarkdownError {
                line: Some(line.number),
                code: "MARKDOWN_FRONT_MATTER_INVALID".to_string(),
                message: "front matter key cannot be empty".to_string(),
            });
        }
        let raw_value = raw_value.trim();
        while object_path
            .last()
            .is_some_and(|(level, _)| indent <= *level)
        {
            object_path.pop();
        }
        if indent > 0 && object_path.is_empty() {
            return Err(MarkdownError {
                line: Some(line.number),
                code: "MARKDOWN_FRONT_MATTER_INVALID".to_string(),
                message: "indented front matter entry has no parent key".to_string(),
            });
        }
        let path = object_path
            .iter()
            .map(|(_, key)| key.as_str())
            .collect::<Vec<_>>();
        let target = front_matter_object_mut(&mut object, &path).ok_or_else(|| MarkdownError {
            line: Some(line.number),
            code: "MARKDOWN_FRONT_MATTER_INVALID".to_string(),
            message: "front matter parent must be an object".to_string(),
        })?;
        if raw_value.is_empty() {
            target.insert(key.to_string(), Value::Object(Map::new()));
            object_path.push((indent, key.to_string()));
        } else {
            target.insert(key.to_string(), yaml_scalar(raw_value));
        }
    }
    Ok((object, end + 1))
}

fn front_matter_object_mut<'a>(
    root: &'a mut Map<String, Value>,
    path: &[&str],
) -> Option<&'a mut Map<String, Value>> {
    let mut current = root;
    for key in path {
        current = current.get_mut(*key)?.as_object_mut()?;
    }
    Some(current)
}

fn yaml_scalar(source: &str) -> Value {
    let source = source.trim();
    if let Some(value) = source
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .or_else(|| {
            source
                .strip_prefix('\'')
                .and_then(|value| value.strip_suffix('\''))
        })
    {
        return json!(value);
    }
    match source.to_ascii_lowercase().as_str() {
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        "null" | "~" => Value::Null,
        _ => source
            .parse::<i64>()
            .map(Value::from)
            .or_else(|_| source.parse::<f64>().map(Value::from))
            .unwrap_or_else(|_| json!(source)),
    }
}

fn pptx_spec(
    parsed: ParsedMarkdown,
    source_name: &str,
) -> Result<MarkdownConversion, MarkdownError> {
    let ParsedMarkdown {
        front_matter,
        blocks,
        mut warnings,
    } = parsed;
    let split = front_matter
        .get("split")
        .and_then(Value::as_str)
        .unwrap_or("both");
    if !matches!(
        split,
        "both" | "heading" | "headings" | "h1" | "rule" | "separator"
    ) {
        return Err(MarkdownError {
            line: Some(2),
            code: "MARKDOWN_FRONT_MATTER_INVALID".to_string(),
            message: "pptx split must be both, heading, or rule".to_string(),
        });
    }
    let split_heading = matches!(split, "both" | "heading" | "headings" | "h1");
    let split_rule = matches!(split, "both" | "rule" | "separator");
    let mut slides = Vec::<Vec<Block>>::new();
    let mut current = Vec::new();
    for block in blocks {
        let boundary = (split_heading && matches!(block, Block::Heading { level: 1, .. }))
            || (split_rule && matches!(block, Block::Rule { .. }));
        if boundary && !current.is_empty() {
            slides.push(std::mem::take(&mut current));
        }
        if matches!(block, Block::Rule { .. }) && split_rule {
            continue;
        }
        current.push(block);
    }
    if !current.is_empty() {
        slides.push(current);
    }
    if slides.is_empty() {
        return Err(MarkdownError {
            line: None,
            code: "MARKDOWN_EMPTY".to_string(),
            message: format!("{source_name} contains no slide content"),
        });
    }

    let mut slide_specs = Vec::new();
    for (index, blocks) in slides.into_iter().enumerate() {
        slide_specs.push(pptx_slide(index, blocks, &mut warnings)?);
    }
    let mut spec = Map::from_iter([
        ("schemaVersion".to_string(), json!(1)),
        ("family".to_string(), json!("pptx")),
        ("slides".to_string(), Value::Array(slide_specs)),
    ]);
    for key in [
        "theme",
        "themeSeed",
        "template",
        "brand",
        "size",
        "footer",
        "slideNumbers",
    ] {
        if let Some(value) = front_matter.get(key) {
            spec.insert(key.to_string(), value.clone());
        }
    }
    for key in front_matter.keys() {
        if !matches!(
            key.as_str(),
            "theme"
                | "themeSeed"
                | "template"
                | "brand"
                | "size"
                | "footer"
                | "slideNumbers"
                | "split"
        ) {
            warnings.push(warning(
                2,
                "MARKDOWN_FRONT_MATTER_UNSUPPORTED",
                format!("PPTX front matter key {key:?} was not mapped"),
            ));
        }
    }
    Ok(MarkdownConversion {
        spec: Value::Object(spec),
        warnings,
    })
}

fn pptx_slide(
    index: usize,
    blocks: Vec<Block>,
    warnings: &mut Vec<MarkdownWarning>,
) -> Result<Value, MarkdownError> {
    let first_line = blocks.first().map(Block::line).unwrap_or(index + 1);
    let mut title = None;
    let mut subtitle = None;
    let mut subtitle_candidate = None;
    let mut paragraphs = Vec::new();
    let mut images = Vec::new();
    let mut tables = Vec::new();
    let mut charts = Vec::new();
    let mut notes = Vec::new();
    let mut first_media_line = None;

    for block in blocks {
        match block {
            Block::Heading { line, content, .. } if title.is_none() => {
                if content.runs.iter().any(inline_run_has_style) {
                    warnings.push(warning(
                        line,
                        "MARKDOWN_TITLE_STYLE_FLATTENED",
                        "PPTX build titles are plain text; inline title styling was flattened",
                    ));
                }
                title = Some(content.text);
            }
            Block::Heading {
                line,
                level,
                content,
            } => {
                warnings.push(warning(
                    line,
                    "MARKDOWN_HEADING_AS_BODY",
                    format!(
                        "level-{level} heading after the slide title was mapped as bold body text"
                    ),
                ));
                paragraphs.push(paragraph_json(
                    content,
                    0,
                    false,
                    false,
                    Some(true),
                    warnings,
                    line,
                ));
            }
            Block::Paragraph { line, content } => {
                if index == 0 && subtitle_candidate.is_none() && paragraphs.is_empty() {
                    subtitle_candidate = Some((line, content));
                } else {
                    paragraphs.push(paragraph_json(
                        content, 0, false, false, None, warnings, line,
                    ));
                }
            }
            Block::ListItem {
                line,
                level,
                ordered,
                content,
            } => paragraphs.push(paragraph_json(
                content, level, !ordered, ordered, None, warnings, line,
            )),
            Block::Image {
                line,
                alt,
                path,
                width,
            } => {
                first_media_line.get_or_insert(line);
                if width.is_some() {
                    warnings.push(warning(
                        line,
                        "MARKDOWN_IMAGE_WIDTH_IGNORED_FOR_PPTX",
                        "image width is a DOCX flow hint; PPTX uses the selected content slot",
                    ));
                }
                images.push(json!({"path": path, "altText": alt}));
            }
            Block::Table { line, rows } => {
                first_media_line.get_or_insert(line);
                tables.push(json!({
                    "rows": rows,
                    "header": true,
                    "bandedRows": true,
                    "style": "Medium2"
                }));
            }
            Block::Code {
                line,
                language,
                text,
            } if language == "chart" => {
                first_media_line.get_or_insert(line);
                let chart: Value = serde_json::from_str(&text).map_err(|error| MarkdownError {
                    line: Some(line),
                    code: "MARKDOWN_CHART_JSON_INVALID".to_string(),
                    message: format!("invalid JSON in chart fence: {error}"),
                })?;
                if !chart.is_object() {
                    return Err(MarkdownError {
                        line: Some(line),
                        code: "MARKDOWN_CHART_JSON_INVALID".to_string(),
                        message: "chart fence JSON must be an object".to_string(),
                    });
                }
                charts.push(chart);
            }
            Block::Code {
                line,
                language,
                text,
            } => {
                warnings.push(warning(
                    line,
                    "MARKDOWN_CODE_BLOCK_AS_TEXT",
                    format!(
                        "{} fenced code block was mapped as a monospace body paragraph",
                        if language.is_empty() {
                            "untyped"
                        } else {
                            language.as_str()
                        }
                    ),
                ));
                paragraphs.push(json!({
                    "text": text,
                    "bullet": false,
                    "runs": [{"text": text, "inlineCode": true}]
                }));
            }
            Block::Notes { text, .. } => notes.push(text),
            Block::Rule { line } => warnings.push(warning(
                line,
                "MARKDOWN_RULE_NOT_A_SLIDE_BOUNDARY",
                "horizontal rule was not a slide boundary under the selected split mode",
            )),
        }
    }

    let media_count = images.len() + tables.len() + charts.len();
    if let Some((line, content)) = subtitle_candidate {
        if paragraphs.is_empty() && media_count == 0 {
            if content.runs.iter().any(inline_run_has_style) {
                warnings.push(warning(
                    line,
                    "MARKDOWN_SUBTITLE_STYLE_FLATTENED",
                    "PPTX build subtitles are plain text; inline subtitle styling was flattened",
                ));
            }
            subtitle = Some(content.text);
        } else {
            paragraphs.insert(
                0,
                paragraph_json(content, 0, false, false, None, warnings, line),
            );
        }
    }

    if title.is_none() {
        title = Some(format!("Slide {}", index + 1));
        warnings.push(warning(
            first_line,
            "MARKDOWN_SLIDE_TITLE_SYNTHESIZED",
            "slide had no heading; a deterministic title was synthesized",
        ));
    }

    if !paragraphs.is_empty() && media_count > 1 {
        return Err(MarkdownError {
            line: first_media_line,
            code: "MARKDOWN_PPTX_MEDIA_DENSITY".to_string(),
            message: "a PPTX slide with body text supports one table, chart, or image; add a slide boundary before additional media"
                .to_string(),
        });
    }
    let layout = if index == 0
        && subtitle.is_some()
        && paragraphs.is_empty()
        && images.is_empty()
        && tables.is_empty()
        && charts.is_empty()
    {
        "Title Slide"
    } else if !paragraphs.is_empty() && media_count > 0 {
        "Two Content"
    } else if paragraphs.is_empty() && media_count == 0 {
        "Section Header"
    } else if paragraphs.is_empty() {
        "Title Only"
    } else {
        "Title and Content"
    };
    let media_slots = pptx_media_slots(media_count, !paragraphs.is_empty());
    let mut media_slot_index = 0;
    for image in &mut images {
        image
            .as_object_mut()
            .expect("image object")
            .insert("slot".to_string(), json!(media_slots[media_slot_index]));
        media_slot_index += 1;
        image
            .as_object_mut()
            .expect("image object")
            .insert("fit".to_string(), json!("contain"));
    }
    for table in &mut tables {
        table
            .as_object_mut()
            .expect("table object")
            .insert("slot".to_string(), json!(media_slots[media_slot_index]));
        media_slot_index += 1;
    }
    for chart in &mut charts {
        let chart = chart.as_object_mut().expect("chart object");
        if !chart.contains_key("bounds") {
            chart
                .entry("slot".to_string())
                .or_insert_with(|| json!(media_slots[media_slot_index]));
        }
        media_slot_index += 1;
    }

    let mut slide = Map::from_iter([
        (
            "id".to_string(),
            json!(format!("markdown-slide-{}", index + 1)),
        ),
        ("layout".to_string(), json!(layout)),
        ("title".to_string(), json!(title.expect("title present"))),
    ]);
    if let Some(subtitle) = subtitle {
        slide.insert("subtitle".to_string(), json!(subtitle));
    }
    if !paragraphs.is_empty() {
        slide.insert("bullets".to_string(), Value::Array(paragraphs));
    }
    if !images.is_empty() {
        slide.insert("images".to_string(), Value::Array(images));
    }
    if !tables.is_empty() {
        slide.insert("tables".to_string(), Value::Array(tables));
    }
    if !charts.is_empty() {
        slide.insert("charts".to_string(), Value::Array(charts));
    }
    if !notes.is_empty() {
        slide.insert("notes".to_string(), json!(notes.join("\n")));
    }
    Ok(Value::Object(slide))
}

fn pptx_media_slots(count: usize, beside_text: bool) -> Vec<String> {
    if count == 0 {
        return Vec::new();
    }
    if beside_text {
        return vec!["right".to_string()];
    }
    if count == 1 {
        return vec!["body".to_string()];
    }
    let mut columns = 1usize;
    while columns * columns < count {
        columns += 1;
    }
    let rows = count.div_ceil(columns);
    (1..=count)
        .map(|index| format!("grid:{rows}x{columns}:{index}"))
        .collect()
}

fn docx_spec(
    parsed: ParsedMarkdown,
    source_name: &str,
) -> Result<MarkdownConversion, MarkdownError> {
    let ParsedMarkdown {
        front_matter,
        blocks,
        mut warnings,
    } = parsed;
    let mut output = Vec::new();
    for block in blocks {
        let line = block.line();
        match block {
            Block::Heading { level, content, .. } => {
                if level > 4 {
                    warnings.push(warning(
                        line,
                        "MARKDOWN_HEADING_LEVEL_CLAMPED",
                        format!("level-{level} heading was mapped to DOCX heading level 4"),
                    ));
                }
                output.push(docx_text_block(
                    "heading",
                    Some(level.min(4)),
                    content,
                    true,
                    &mut warnings,
                    line,
                ));
            }
            Block::Paragraph { content, .. } => output.push(docx_text_block(
                "paragraph",
                None,
                content,
                true,
                &mut warnings,
                line,
            )),
            Block::ListItem {
                level,
                ordered,
                content,
                ..
            } => output.push(docx_text_block(
                if ordered { "numbered" } else { "bullet" },
                Some(level),
                content,
                false,
                &mut warnings,
                line,
            )),
            Block::Image {
                alt, path, width, ..
            } => {
                let mut image = Map::from_iter([
                    ("path".to_string(), json!(path)),
                    ("altText".to_string(), json!(alt)),
                ]);
                if let Some(width) = width {
                    image.insert("width".to_string(), json!(width));
                }
                output.push(json!({"type": "image", "image": image}));
            }
            Block::Table { rows, .. } => output.push(json!({
                "type": "table",
                "table": {"rows": rows, "header": true, "bandedRows": true}
            })),
            Block::Rule { .. } => output.push(json!({"type": "pageBreak"})),
            Block::Code { text, .. } => output.push(json!({
                "type": "paragraph",
                "text": text,
                "runs": [{"text": text, "inlineCode": true}]
            })),
            Block::Notes { text, .. } => {
                warnings.push(warning(
                    line,
                    "MARKDOWN_NOTES_AS_PARAGRAPH",
                    "notes syntax is PPTX-specific; DOCX preserved it as a paragraph",
                ));
                output.push(json!({"type": "paragraph", "text": text}));
            }
        }
    }
    if output.is_empty() {
        return Err(MarkdownError {
            line: None,
            code: "MARKDOWN_EMPTY".to_string(),
            message: format!("{source_name} contains no document content"),
        });
    }
    let mut spec = Map::from_iter([
        ("schemaVersion".to_string(), json!(1)),
        ("family".to_string(), json!("docx")),
        ("blocks".to_string(), Value::Array(output)),
    ]);
    for key in [
        "template",
        "theme",
        "themeSeed",
        "brand",
        "title",
        "subtitle",
        "sections",
    ] {
        if let Some(value) = front_matter.get(key) {
            spec.insert(key.to_string(), value.clone());
        }
    }
    let mut metadata = front_matter
        .get("metadata")
        .cloned()
        .unwrap_or_else(|| json!({}));
    if let Some(author) = front_matter.get("author") {
        let author = author.as_str().ok_or_else(|| MarkdownError {
            line: Some(2),
            code: "MARKDOWN_FRONT_MATTER_INVALID".to_string(),
            message: "DOCX author front matter must be a string".to_string(),
        })?;
        metadata
            .as_object_mut()
            .ok_or_else(|| MarkdownError {
                line: Some(2),
                code: "MARKDOWN_FRONT_MATTER_INVALID".to_string(),
                message: "DOCX metadata front matter must be an object".to_string(),
            })?
            .insert("creator".to_string(), json!(author));
    }
    if metadata
        .as_object()
        .is_some_and(|metadata| !metadata.is_empty())
    {
        spec.insert("metadata".to_string(), metadata);
    }
    let headers = document_region_front_matter(&front_matter, "headers", "header", false)?;
    if !headers.is_empty() {
        spec.insert("headers".to_string(), Value::Object(headers));
    }
    let footers = document_region_front_matter(&front_matter, "footers", "footer", true)?;
    if !footers.is_empty() {
        spec.insert("footers".to_string(), Value::Object(footers));
    }
    let toc = match front_matter.get("toc") {
        Some(Value::Bool(value)) => Some(*value),
        Some(_) => {
            return Err(MarkdownError {
                line: Some(2),
                code: "MARKDOWN_FRONT_MATTER_INVALID".to_string(),
                message: "DOCX toc front matter must be a boolean".to_string(),
            });
        }
        None => None,
    };
    if toc == Some(true) {
        spec.get_mut("blocks")
            .and_then(Value::as_array_mut)
            .expect("blocks array")
            .insert(0, json!({"type": "toc"}));
    }
    for key in front_matter.keys() {
        if !matches!(
            key.as_str(),
            "template"
                | "theme"
                | "themeSeed"
                | "brand"
                | "title"
                | "subtitle"
                | "author"
                | "metadata"
                | "header"
                | "headers"
                | "footer"
                | "footers"
                | "pageNumbers"
                | "sections"
                | "toc"
                | "pageSetup"
        ) {
            warnings.push(warning(
                2,
                "MARKDOWN_FRONT_MATTER_UNSUPPORTED",
                format!("DOCX front matter key {key:?} was not mapped"),
            ));
        }
    }
    if let Some(page_setup) = front_matter.get("pageSetup") {
        if front_matter.contains_key("sections") {
            return Err(MarkdownError {
                line: Some(2),
                code: "MARKDOWN_FRONT_MATTER_INVALID".to_string(),
                message: "DOCX pageSetup and sections front matter are mutually exclusive"
                    .to_string(),
            });
        }
        let setup = page_setup.as_object().ok_or_else(|| MarkdownError {
            line: Some(2),
            code: "MARKDOWN_FRONT_MATTER_INVALID".to_string(),
            message: "pageSetup front matter must be an object".to_string(),
        })?;
        let mut section = setup.clone();
        section.insert("startBlock".to_string(), json!(0));
        spec.insert("sections".to_string(), json!([section]));
    }
    Ok(MarkdownConversion {
        spec: Value::Object(spec),
        warnings,
    })
}

fn docx_text_block(
    kind: &str,
    level: Option<u8>,
    content: InlineContent,
    supports_runs: bool,
    warnings: &mut Vec<MarkdownWarning>,
    line: usize,
) -> Value {
    let text = content.text.clone();
    let runs = spec_runs(content, warnings, line);
    let rich = runs.iter().any(spec_run_has_style);
    let mut block = Map::from_iter([
        ("type".to_string(), json!(kind)),
        ("text".to_string(), json!(text)),
    ]);
    if let Some(level) = level {
        block.insert("level".to_string(), json!(level));
    }
    if rich && supports_runs {
        block.insert("runs".to_string(), Value::Array(runs));
    } else if rich {
        warnings.push(warning(
            line,
            "MARKDOWN_LIST_INLINE_STYLE_FLATTENED",
            "DOCX list operations preserve list structure and text but do not support inline run styling",
        ));
    }
    Value::Object(block)
}

fn spec_run_has_style(run: &Value) -> bool {
    run.as_object().is_some_and(|run| run.len() > 1)
}

fn document_region_front_matter(
    front_matter: &Map<String, Value>,
    plural: &str,
    singular: &str,
    include_page_numbers: bool,
) -> Result<Map<String, Value>, MarkdownError> {
    let mut region = match front_matter.get(plural) {
        Some(Value::Object(region)) => region.clone(),
        Some(_) => {
            return Err(MarkdownError {
                line: Some(2),
                code: "MARKDOWN_FRONT_MATTER_INVALID".to_string(),
                message: format!("DOCX {plural} front matter must be an object"),
            });
        }
        None => Map::new(),
    };
    for key in region.keys() {
        let supported = matches!(key.as_str(), "default" | "first" | "even")
            || (include_page_numbers && key == "pageNumbers");
        if !supported {
            return Err(MarkdownError {
                line: Some(2),
                code: "MARKDOWN_FRONT_MATTER_INVALID".to_string(),
                message: format!("unsupported DOCX {plural} key {key:?}"),
            });
        }
    }
    if let Some(value) = front_matter.get(singular) {
        if region.contains_key("default") {
            return Err(MarkdownError {
                line: Some(2),
                code: "MARKDOWN_FRONT_MATTER_INVALID".to_string(),
                message: format!(
                    "DOCX {singular} and {plural}.default front matter are mutually exclusive"
                ),
            });
        }
        let text = value.as_str().ok_or_else(|| MarkdownError {
            line: Some(2),
            code: "MARKDOWN_FRONT_MATTER_INVALID".to_string(),
            message: format!("DOCX {singular} front matter must be a string"),
        })?;
        region.insert("default".to_string(), json!(text));
    }
    if include_page_numbers && let Some(value) = front_matter.get("pageNumbers") {
        if region.contains_key("pageNumbers") {
            return Err(MarkdownError {
                line: Some(2),
                code: "MARKDOWN_FRONT_MATTER_INVALID".to_string(),
                message:
                    "DOCX pageNumbers and footers.pageNumbers front matter are mutually exclusive"
                        .to_string(),
            });
        }
        let enabled = value.as_bool().ok_or_else(|| MarkdownError {
            line: Some(2),
            code: "MARKDOWN_FRONT_MATTER_INVALID".to_string(),
            message: "DOCX pageNumbers front matter must be a boolean".to_string(),
        })?;
        region.insert("pageNumbers".to_string(), json!(enabled));
    }
    Ok(region)
}

fn paragraph_json(
    content: InlineContent,
    level: u8,
    bullet: bool,
    numbered: bool,
    bold: Option<bool>,
    warnings: &mut Vec<MarkdownWarning>,
    line: usize,
) -> Value {
    let mut paragraph = Map::from_iter([
        ("text".to_string(), json!(content.text)),
        ("level".to_string(), json!(level)),
        ("bullet".to_string(), json!(bullet || numbered)),
    ]);
    if numbered {
        warnings.push(warning(
            line,
            "MARKDOWN_NUMBERING_RENDERED_AS_BULLET",
            "PPTX body numbering is not available; ordered list item was preserved as a bullet",
        ));
    }
    if let Some(bold) = bold {
        paragraph.insert("bold".to_string(), json!(bold));
    }
    let runs = spec_runs(content, warnings, line);
    if !runs.is_empty() {
        paragraph.insert("runs".to_string(), Value::Array(runs));
    }
    Value::Object(paragraph)
}

fn spec_runs(
    content: InlineContent,
    _warnings: &mut Vec<MarkdownWarning>,
    _line: usize,
) -> Vec<Value> {
    content
        .runs
        .into_iter()
        .map(|run| {
            let mut output = Map::from_iter([("text".to_string(), json!(run.text))]);
            if run.bold {
                output.insert("bold".to_string(), json!(true));
            }
            if run.italic {
                output.insert("italic".to_string(), json!(true));
            }
            if let Some(link) = run.link {
                output.insert("link".to_string(), json!(link));
            }
            if run.code {
                output.insert("inlineCode".to_string(), json!(true));
            }
            Value::Object(output)
        })
        .collect()
}

fn parse_inlines(source: &str) -> InlineContent {
    let mut runs = Vec::new();
    let mut cursor = 0usize;
    while cursor < source.len() {
        let rest = &source[cursor..];
        if let Some(after) = rest.strip_prefix('\\')
            && let Some(escaped) = after.chars().next()
            && escaped.is_ascii_punctuation()
        {
            let mut text = [0; 4];
            push_inline_run(
                &mut runs,
                escaped.encode_utf8(&mut text),
                false,
                false,
                false,
                None,
            );
            cursor += 1 + escaped.len_utf8();
            continue;
        }
        if let Some(after) = rest.strip_prefix("**")
            && let Some(end) = after.find("**")
        {
            push_inline_run(&mut runs, &after[..end], true, false, false, None);
            cursor += 2 + end + 2;
            continue;
        }
        if let Some(after) = rest.strip_prefix("__")
            && let Some(end) = after.find("__")
        {
            push_inline_run(&mut runs, &after[..end], true, false, false, None);
            cursor += 2 + end + 2;
            continue;
        }
        if let Some(after) = rest.strip_prefix('`')
            && let Some(end) = after.find('`')
        {
            push_inline_run(&mut runs, &after[..end], false, false, true, None);
            cursor += 1 + end + 1;
            continue;
        }
        if let Some(after) = rest.strip_prefix('[')
            && let Some(label_end) = after.find("](")
        {
            let url_start = label_end + 2;
            if let Some(url_end) = after[url_start..].find(')') {
                let label = &after[..label_end];
                let url = &after[url_start..url_start + url_end];
                push_inline_run(&mut runs, label, false, false, false, Some(url.to_string()));
                cursor += 1 + url_start + url_end + 1;
                continue;
            }
        }
        if let Some(after) = rest.strip_prefix('*')
            && let Some(end) = after.find('*')
        {
            push_inline_run(&mut runs, &after[..end], false, true, false, None);
            cursor += 1 + end + 1;
            continue;
        }
        if let Some(after) = rest.strip_prefix('_')
            && let Some(end) = after.find('_')
        {
            push_inline_run(&mut runs, &after[..end], false, true, false, None);
            cursor += 1 + end + 1;
            continue;
        }

        let next = rest
            .char_indices()
            .skip(1)
            .find_map(|(index, ch)| matches!(ch, '\\' | '*' | '_' | '`' | '[').then_some(index))
            .unwrap_or(rest.len());
        push_inline_run(&mut runs, &rest[..next], false, false, false, None);
        cursor += next;
    }
    InlineContent {
        text: runs.iter().map(|run| run.text.as_str()).collect(),
        runs,
    }
}

fn push_inline_run(
    runs: &mut Vec<InlineRun>,
    text: &str,
    bold: bool,
    italic: bool,
    code: bool,
    link: Option<String>,
) {
    if text.is_empty() {
        return;
    }
    if let Some(previous) = runs.last_mut()
        && previous.bold == bold
        && previous.italic == italic
        && previous.code == code
        && previous.link == link
    {
        previous.text.push_str(text);
        return;
    }
    runs.push(InlineRun {
        text: text.to_string(),
        bold,
        italic,
        code,
        link,
    });
}

fn inline_run_has_style(run: &InlineRun) -> bool {
    run.bold || run.italic || run.code || run.link.is_some()
}

fn heading(source: &str) -> Option<(u8, &str)> {
    let hashes = source.chars().take_while(|ch| *ch == '#').count();
    if !(1..=6).contains(&hashes) || source.as_bytes().get(hashes) != Some(&b' ') {
        return None;
    }
    Some((hashes as u8, source[hashes + 1..].trim()))
}

fn list_item(source: &str) -> Option<(u8, bool, &str)> {
    let indent = source
        .chars()
        .take_while(|ch| matches!(ch, ' ' | '\t'))
        .fold(0usize, |total, ch| total + if ch == '\t' { 2 } else { 1 });
    let body = source.trim_start();
    if let Some(text) = body.strip_prefix("- ").or_else(|| body.strip_prefix("* ")) {
        return Some(((indent / 2).min(8) as u8, false, text));
    }
    let digits = body.chars().take_while(|ch| ch.is_ascii_digit()).count();
    if digits > 0 && body[digits..].starts_with(". ") {
        return Some(((indent / 2).min(8) as u8, true, &body[digits + 2..]));
    }
    None
}

fn standalone_image(source: &str) -> Option<(String, String, Option<String>)> {
    let after = source.strip_prefix("![")?;
    let alt_end = after.find("](")?;
    let path_start = alt_end + 2;
    let path_end = after[path_start..].find(')')? + path_start;
    let suffix = after[path_end + 1..].trim();
    if !suffix.is_empty() && !(suffix.starts_with('{') && suffix.ends_with('}')) {
        return None;
    }
    let width = suffix
        .strip_prefix('{')
        .and_then(|value| value.strip_suffix('}'))
        .and_then(|attributes| {
            attributes.split_whitespace().find_map(|attribute| {
                attribute
                    .strip_prefix("width=")
                    .map(|value| value.trim_matches(['"', '\'']).to_string())
            })
        });
    Some((
        after[..alt_end].to_string(),
        after[path_start..path_end].to_string(),
        width,
    ))
}

fn is_rule(source: &str) -> bool {
    matches!(source, "---" | "***" | "___")
}

fn is_slide_boundary(source: &str) -> bool {
    is_rule(source) || heading(source).is_some_and(|(level, _)| level == 1)
}

fn looks_like_table_row(source: &str) -> bool {
    source.contains('|') && table_cells(source).len() >= 2
}

fn is_table_separator(source: &str) -> bool {
    let cells = table_cells(source);
    !cells.is_empty()
        && cells.iter().all(|cell| {
            let cell = cell.trim().trim_matches(':');
            cell.len() >= 3 && cell.bytes().all(|byte| byte == b'-')
        })
}

fn table_cells(source: &str) -> Vec<String> {
    source
        .trim()
        .trim_start_matches('|')
        .trim_end_matches('|')
        .split('|')
        .map(|cell| cell.trim().to_string())
        .collect()
}

fn is_block_start(lines: &[SourceLine<'_>], index: usize) -> bool {
    let source = lines[index].text.trim();
    source.starts_with("```")
        || source.starts_with("<!--")
        || source.eq_ignore_ascii_case("notes:")
        || heading(source).is_some()
        || is_rule(source)
        || list_item(lines[index].text).is_some()
        || standalone_image(source).is_some()
        || (index + 1 < lines.len()
            && looks_like_table_row(source)
            && is_table_separator(lines[index + 1].text.trim()))
}

fn warning(line: usize, code: &str, message: impl Into<String>) -> MarkdownWarning {
    MarkdownWarning {
        line,
        code: code.to_string(),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pptx_conversion_maps_front_matter_slides_rich_runs_table_chart_image_and_notes() {
        let source = r#"---
theme: corporate
size: 16:9
footer: Internal
slideNumbers: true
split: heading
---
# Q3 Review
Results and decisions
# Performance
- **Revenue** grew
  - See [dashboard](https://example.test/q3)

# Results Table
| Quarter | Actual |
| --- | ---: |
| Q3 | 45 |

# Results Chart
```chart
{"type":"column","categories":["Q3"],"series":[{"name":"Actual","values":[45]}]}
```

# Product Detail
![Chart detail](chart.png)
<!-- notes: Explain the Q3 variance. -->
"#;
        let conversion = markdown_to_spec(BuildFamily::Pptx, source, "review.md")
            .expect("convert PPTX Markdown");
        assert_eq!(conversion.spec["theme"], "corporate");
        assert_eq!(conversion.spec["size"], "16:9");
        assert_eq!(conversion.spec["footer"], "Internal");
        assert_eq!(conversion.spec["slideNumbers"], true);
        let slides = conversion.spec["slides"].as_array().unwrap();
        assert_eq!(slides.len(), 5);
        assert_eq!(slides[0]["layout"], "Title Slide");
        assert_eq!(slides[1]["layout"], "Title and Content");
        assert_eq!(slides[1]["bullets"][0]["runs"][0]["bold"], true);
        assert_eq!(
            slides[1]["bullets"][1]["runs"][1]["link"],
            "https://example.test/q3"
        );
        assert_eq!(slides[2]["tables"][0]["rows"][1][1], "45");
        assert_eq!(slides[3]["charts"][0]["type"], "column");
        assert_eq!(slides[4]["images"][0]["altText"], "Chart detail");
        assert_eq!(slides[4]["notes"], "Explain the Q3 variance.");
        assert!(conversion.warnings.is_empty());
    }

    #[test]
    fn docx_conversion_maps_flow_blocks_and_page_breaks() {
        let source = r#"---
title: Quarterly Report
author: Ada Lovelace
header: Q3 Review
footer: Internal
pageNumbers: true
toc: true
pageSetup:
  size: A4
  margins:
    top: 0.75in
    bottom: 0.75in
---
# Report

Paragraph with *emphasis*.

1. First
  1. Nested

---

```rust
let x = 1;
```
"#;
        let conversion = markdown_to_spec(BuildFamily::Docx, source, "report.md")
            .expect("convert DOCX Markdown");
        let blocks = conversion.spec["blocks"].as_array().unwrap();
        assert_eq!(conversion.spec["metadata"]["creator"], "Ada Lovelace");
        assert_eq!(conversion.spec["headers"]["default"], "Q3 Review");
        assert_eq!(conversion.spec["footers"]["default"], "Internal");
        assert_eq!(conversion.spec["footers"]["pageNumbers"], true);
        assert_eq!(conversion.spec["sections"][0]["size"], "A4");
        assert_eq!(conversion.spec["sections"][0]["margins"]["top"], "0.75in");
        assert_eq!(blocks[0]["type"], "toc");
        assert_eq!(blocks[1]["type"], "heading");
        assert_eq!(blocks[2]["runs"][1]["italic"], true);
        assert_eq!(blocks[3]["type"], "numbered");
        assert_eq!(blocks[4]["level"], 1);
        assert_eq!(blocks[5]["type"], "pageBreak");
        assert_eq!(blocks[6]["runs"][0]["inlineCode"], true);
    }

    #[test]
    fn unsupported_syntax_is_preserved_with_a_source_line_warning() {
        let conversion = markdown_to_spec(
            BuildFamily::Pptx,
            "# Quote\n\n> retained words\n",
            "quote.md",
        )
        .expect("convert blockquote");
        assert_eq!(conversion.spec["slides"][0]["subtitle"], "> retained words");
        assert!(conversion.warnings.iter().any(|warning| {
            warning.line == 3 && warning.code == "MARKDOWN_BLOCKQUOTE_UNSUPPORTED"
        }));
    }

    #[test]
    fn invalid_chart_json_reports_the_fence_line() {
        let error = markdown_to_spec(
            BuildFamily::Pptx,
            "# Chart\n\n```chart\nnot json\n```\n",
            "chart.md",
        )
        .unwrap_err();
        assert_eq!(error.line, Some(3));
        assert_eq!(error.code, "MARKDOWN_CHART_JSON_INVALID");
    }

    #[test]
    fn clean_conversion_omits_an_empty_warnings_member() {
        let conversion = markdown_to_spec(BuildFamily::Pptx, "# Title\n", "clean.md")
            .expect("convert clean Markdown");
        let serialized = serde_json::to_value(conversion).expect("serialize conversion");
        assert!(serialized.get("warnings").is_none());
    }

    #[test]
    fn json_front_matter_selects_rule_based_slide_splitting() {
        let source =
            "---\n{\"theme\":\"warm\",\"split\":\"rule\"}\n---\n# First\n\n---\n\n# Second\n";
        let conversion = markdown_to_spec(BuildFamily::Pptx, source, "json-front-matter.md")
            .expect("convert JSON-front-matter Markdown");
        assert_eq!(conversion.spec["theme"], "warm");
        assert_eq!(conversion.spec["slides"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn unsupported_family_is_an_explicit_teaching_error() {
        let error = markdown_to_spec(BuildFamily::Xlsx, "# Data\n", "data.md").unwrap_err();
        assert_eq!(error.line, None);
        assert_eq!(error.code, "MARKDOWN_FAMILY_UNSUPPORTED");
        assert!(error.message.contains("pptx and docx"));
    }

    #[test]
    fn media_only_slides_use_a_grid_and_dense_mixed_slides_are_refused() {
        let gallery = markdown_to_spec(
            BuildFamily::Pptx,
            "# Gallery\n\n![One](one.png)\n\n![Two](two.png)\n",
            "gallery.md",
        )
        .expect("convert media gallery");
        assert_eq!(gallery.spec["slides"][0]["layout"], "Title Only");
        assert_eq!(gallery.spec["slides"][0]["images"][0]["slot"], "grid:1x2:1");
        assert_eq!(gallery.spec["slides"][0]["images"][1]["slot"], "grid:1x2:2");

        let dense = markdown_to_spec(
            BuildFamily::Pptx,
            "# Dense\n\nContext\n\n![One](one.png)\n\n![Two](two.png)\n",
            "dense.md",
        )
        .unwrap_err();
        assert_eq!(dense.line, Some(5));
        assert_eq!(dense.code, "MARKDOWN_PPTX_MEDIA_DENSITY");
    }
}
