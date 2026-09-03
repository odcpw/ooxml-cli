use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use serde_json::Value;
use std::collections::BTreeMap;

use crate::cli_core::{CliError, CliResult, GlobalFlags};
use crate::{
    attr, decode_xml_text, docx_body_block_ranges, docx_body_tag, local_name, pptx_extract_notes,
    pptx_tables_show, relationships, resolve_relationship_target, zip_text,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MarkdownCommand {
    DocxText,
    PptxText,
    XlsxRange,
    Outline,
}

pub(crate) struct MarkdownRequest {
    pub(crate) args: Vec<String>,
    pub(crate) formatted: bool,
    command: MarkdownCommand,
}

pub(crate) fn normalize_request(
    flags: &GlobalFlags,
    args: &[String],
) -> CliResult<Option<MarkdownRequest>> {
    let command = markdown_command(args);
    let mut normalized = Vec::with_capacity(args.len());
    let mut local_markdown = false;
    let mut formatted = false;
    let mut index = 0;
    while index < args.len() {
        if command.is_some() && matches!(args[index].as_str(), "--format" | "-f") {
            let value = args
                .get(index + 1)
                .ok_or_else(|| CliError::invalid_args("--format requires a value"))?;
            if value != "markdown" {
                return Err(CliError::invalid_args(format!(
                    "invalid local format: {value} (expected 'markdown')"
                )));
            }
            local_markdown = true;
            index += 2;
            continue;
        }
        if command.is_some()
            && (args[index].starts_with("--format=") || args[index].starts_with("-f="))
        {
            let (_, value) = args[index]
                .split_once('=')
                .expect("format prefix matched but split failed");
            if value != "markdown" {
                return Err(CliError::invalid_args(format!(
                    "invalid local format: {value} (expected 'markdown')"
                )));
            }
            local_markdown = true;
            index += 1;
            continue;
        }
        if args[index] == "--formatted" && command == Some(MarkdownCommand::XlsxRange) {
            formatted = true;
            index += 1;
            continue;
        }
        normalized.push(args[index].clone());
        index += 1;
    }

    if !local_markdown && !flags.format_markdown {
        if formatted {
            return Err(CliError::invalid_args(
                "--formatted requires --format markdown",
            ));
        }
        return Ok(None);
    }
    let Some(command) = command else {
        return Err(CliError::invalid_args(
            "markdown output is supported only for docx text, pptx extract text, xlsx ranges export, and outline",
        ));
    };
    if flags.json || flags.format_text {
        return Err(CliError::invalid_args(
            "choose exactly one output format: json, text, or markdown",
        ));
    }
    if formatted && !normalized.iter().any(|arg| arg == "--include-formats") {
        normalized.push("--include-formats".to_string());
    }
    Ok(Some(MarkdownRequest {
        args: normalized,
        formatted,
        command,
    }))
}

pub(crate) fn render(request: &MarkdownRequest, value: &Value) -> CliResult<String> {
    let markdown = match request.command {
        MarkdownCommand::DocxText => render_docx(&request.args[2], value)?,
        MarkdownCommand::PptxText => render_pptx(&request.args[3], &request.args[4..], value)?,
        MarkdownCommand::XlsxRange => render_xlsx(value, request.formatted),
        MarkdownCommand::Outline => render_outline(value),
    };
    Ok(markdown.trim_end().to_string() + "\n")
}

fn markdown_command(args: &[String]) -> Option<MarkdownCommand> {
    match args {
        [family, verb, _, ..] if family == "docx" && verb == "text" => {
            Some(MarkdownCommand::DocxText)
        }
        [family, group, verb, _, ..]
            if family == "pptx" && group == "extract" && verb == "text" =>
        {
            Some(MarkdownCommand::PptxText)
        }
        [family, group, verb, _, ..]
            if family == "xlsx" && group == "ranges" && verb == "export" =>
        {
            Some(MarkdownCommand::XlsxRange)
        }
        [command, _, ..] if command == "outline" => Some(MarkdownCommand::Outline),
        _ => None,
    }
}

fn render_docx(file: &str, value: &Value) -> CliResult<String> {
    let xml = zip_text(file, "word/document.xml")?;
    let body_tag = docx_body_tag(&xml)?;
    let ranges = docx_body_block_ranges(&xml, &body_tag)?;
    let rels = relationships(file, "word/_rels/document.xml.rels").unwrap_or_default();
    let list_formats = docx_list_formats(file);
    let blocks = value["blocks"].as_array().cloned().unwrap_or_default();
    let mut output = String::new();
    for (block, range) in blocks.iter().zip(ranges) {
        if block["kind"] == "table" {
            render_pipe_table(&mut output, &table_rows(&block["table"]["rows"]));
            continue;
        }
        let fragment = &xml[range.start..range.end];
        let rich = render_docx_inline(fragment, &rels);
        let text = if rich.trim().is_empty() {
            markdown_escape(block["text"].as_str().unwrap_or_default())
        } else {
            rich
        };
        let style = block["styleId"]
            .as_str()
            .or_else(|| block["style"].as_str())
            .unwrap_or_default();
        if let Some(level) = heading_level(style) {
            output.push_str(&"#".repeat(level));
            output.push(' ');
            output.push_str(&text);
            output.push_str("\n\n");
        } else if let Some(level) = block["listLevel"].as_u64() {
            output.push_str(&"  ".repeat(level as usize));
            let num_id = block["numId"].as_u64().unwrap_or_default() as u32;
            output.push_str(
                if list_formats.get(&(num_id, level as u32)) == Some(&false) {
                    "1. "
                } else {
                    "- "
                },
            );
            output.push_str(&text);
            output.push('\n');
        } else if !text.is_empty() {
            output.push_str(&text);
            output.push_str("\n\n");
        }
    }
    Ok(output)
}

fn docx_list_formats(file: &str) -> BTreeMap<(u32, u32), bool> {
    let Ok(xml) = zip_text(file, "word/numbering.xml") else {
        return BTreeMap::new();
    };
    parse_docx_list_formats(&xml)
}

fn parse_docx_list_formats(xml: &str) -> BTreeMap<(u32, u32), bool> {
    let mut abstract_formats = BTreeMap::<(u32, u32), bool>::new();
    let mut num_to_abstract = BTreeMap::<u32, u32>::new();
    let mut current_abstract = None;
    let mut current_level = None;
    let mut current_num = None;
    let mut reader = Reader::from_str(xml);
    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) => {
                let name = local_name(element.name().as_ref()).to_string();
                match name.as_str() {
                    "abstractNum" => {
                        current_abstract = attr(&element, "abstractNumId")
                            .and_then(|value| value.parse::<u32>().ok());
                    }
                    "lvl" => {
                        current_level =
                            attr(&element, "ilvl").and_then(|value| value.parse::<u32>().ok());
                    }
                    "num" => {
                        current_num =
                            attr(&element, "numId").and_then(|value| value.parse::<u32>().ok());
                    }
                    _ => record_docx_list_element(
                        &name,
                        &element,
                        current_abstract,
                        current_level,
                        current_num,
                        &mut abstract_formats,
                        &mut num_to_abstract,
                    ),
                }
            }
            Ok(Event::Empty(element)) => record_docx_list_element(
                local_name(element.name().as_ref()),
                &element,
                current_abstract,
                current_level,
                current_num,
                &mut abstract_formats,
                &mut num_to_abstract,
            ),
            Ok(Event::End(element)) => match local_name(element.name().as_ref()) {
                "abstractNum" => current_abstract = None,
                "lvl" => current_level = None,
                "num" => current_num = None,
                _ => {}
            },
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    num_to_abstract
        .into_iter()
        .flat_map(|(num_id, abstract_id)| {
            abstract_formats
                .iter()
                .filter_map(move |(&(id, level), &bullet)| {
                    (id == abstract_id).then_some(((num_id, level), bullet))
                })
        })
        .collect()
}

fn record_docx_list_element(
    name: &str,
    element: &BytesStart<'_>,
    current_abstract: Option<u32>,
    current_level: Option<u32>,
    current_num: Option<u32>,
    abstract_formats: &mut BTreeMap<(u32, u32), bool>,
    num_to_abstract: &mut BTreeMap<u32, u32>,
) {
    if name == "numFmt"
        && let (Some(abstract_id), Some(level), Some(format)) =
            (current_abstract, current_level, attr(element, "val"))
    {
        abstract_formats.insert((abstract_id, level), format == "bullet");
    } else if name == "abstractNumId"
        && let (Some(num_id), Some(abstract_id)) = (
            current_num,
            attr(element, "val").and_then(|value| value.parse::<u32>().ok()),
        )
    {
        num_to_abstract.insert(num_id, abstract_id);
    }
}

#[derive(Default)]
struct DocxRun {
    text: String,
    bold: bool,
    italic: bool,
    image_rel: Option<String>,
    image_alt: String,
}

fn render_docx_inline(fragment: &str, rels: &BTreeMap<String, String>) -> String {
    let mut reader = Reader::from_str(fragment);
    reader.config_mut().trim_text(false);
    let mut stack = Vec::<String>::new();
    let mut hyperlink = None::<String>;
    let mut run = None::<DocxRun>;
    let mut output = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) => {
                let name = local_name(element.name().as_ref()).to_string();
                if name == "hyperlink" {
                    hyperlink = attr(&element, "id");
                } else if name == "r" {
                    run = Some(DocxRun::default());
                } else {
                    apply_docx_inline_element(&name, &element, run.as_mut());
                }
                stack.push(name);
            }
            Ok(Event::Empty(element)) => {
                let name = local_name(element.name().as_ref()).to_string();
                apply_docx_inline_element(&name, &element, run.as_mut());
            }
            Ok(Event::Text(text)) if stack.last().is_some_and(|name| name == "t") => {
                if let Some(run) = run.as_mut() {
                    run.text.push_str(&decode_xml_text(text.as_ref()));
                }
            }
            Ok(Event::End(element)) => {
                let name = local_name(element.name().as_ref()).to_string();
                if name == "r"
                    && let Some(run) = run.take()
                {
                    output.push_str(&render_docx_run(run, hyperlink.as_deref(), rels));
                }
                if name == "hyperlink" {
                    hyperlink = None;
                }
                stack.pop();
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    output
}

fn apply_docx_inline_element(name: &str, element: &BytesStart<'_>, run: Option<&mut DocxRun>) {
    let Some(run) = run else {
        return;
    };
    match name {
        "b" => run.bold = on_off_enabled(element),
        "i" => run.italic = on_off_enabled(element),
        "tab" => run.text.push('\t'),
        "br" | "cr" => run.text.push_str("  \n"),
        "blip" => run.image_rel = attr(element, "embed"),
        "docPr" => {
            run.image_alt = attr(element, "descr")
                .or_else(|| attr(element, "title"))
                .or_else(|| attr(element, "name"))
                .unwrap_or_else(|| "image".to_string());
        }
        _ => {}
    }
}

fn on_off_enabled(element: &BytesStart<'_>) -> bool {
    !attr(element, "val").is_some_and(|value| {
        matches!(
            value.to_ascii_lowercase().as_str(),
            "0" | "false" | "off" | "no"
        )
    })
}

fn render_docx_run(
    run: DocxRun,
    hyperlink: Option<&str>,
    rels: &BTreeMap<String, String>,
) -> String {
    let mut text = markdown_escape(&run.text);
    if run.bold && !text.is_empty() {
        text = format!("**{text}**");
    }
    if run.italic && !text.is_empty() {
        text = format!("*{text}*");
    }
    if let Some(target) = hyperlink.and_then(|id| rels.get(id))
        && !text.is_empty()
    {
        text = format!("[{text}]({})", markdown_url(target));
    }
    if let Some(target) = run.image_rel.and_then(|id| rels.get(&id)) {
        let target = resolve_relationship_target("/word/document.xml", target);
        text.push_str(&format!(
            "![{}](ooxml:{})",
            markdown_escape(&run.image_alt),
            markdown_url(&target)
        ));
    }
    text
}

fn heading_level(style: &str) -> Option<usize> {
    let compact = style.replace([' ', '-'], "").to_ascii_lowercase();
    if compact == "title" {
        return Some(1);
    }
    compact
        .strip_prefix("heading")
        .and_then(|level| level.parse::<usize>().ok())
        .filter(|level| (1..=6).contains(level))
}

fn table_rows(rows: &Value) -> Vec<Vec<String>> {
    rows.as_array()
        .into_iter()
        .flatten()
        .map(|row| {
            row["cells"]
                .as_array()
                .into_iter()
                .flatten()
                .map(markdown_cell)
                .collect()
        })
        .collect()
}

fn render_pptx(file: &str, rest: &[String], value: &Value) -> CliResult<String> {
    let notes = pptx_extract_notes(file, rest)?;
    let notes_by_slide = notes["notes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|note| {
            Some((
                note["slide"].as_u64()?,
                note["notes"]["plainText"].as_str()?.to_string(),
            ))
        })
        .collect::<BTreeMap<_, _>>();
    let slides = value["slides"].as_array().cloned().unwrap_or_default();
    let mut output = String::new();
    for (position, slide) in slides.iter().enumerate() {
        if position > 0 {
            if !output.ends_with("\n\n") {
                output.push('\n');
            }
            output.push_str("---\n\n");
        }
        let number = slide["slide"].as_u64().unwrap_or(position as u64 + 1);
        let shapes = slide["shapes"].as_array().cloned().unwrap_or_default();
        let title = shapes
            .iter()
            .find(|shape| shape["key"] == "title")
            .and_then(|shape| shape["text"]["plainText"].as_str())
            .filter(|title| !title.is_empty());
        output.push_str("# ");
        if let Some(title) = title {
            output.push_str(&markdown_escape(title));
        } else {
            output.push_str(&format!("Slide {number}"));
        }
        output.push_str("\n\n");
        for shape in shapes.iter().filter(|shape| shape["key"] != "title") {
            for paragraph in shape["text"]["paragraphs"].as_array().into_iter().flatten() {
                let text = render_json_runs(paragraph);
                if text.is_empty() {
                    continue;
                }
                if paragraph["bullet"].as_bool().unwrap_or(false) {
                    let level = paragraph["level"].as_u64().unwrap_or_default();
                    output.push_str(&"  ".repeat(level as usize));
                    output.push_str("- ");
                    output.push_str(&text);
                    output.push('\n');
                } else {
                    output.push_str(&text);
                    output.push_str("\n\n");
                }
            }
        }
        let tables = pptx_tables_show(file, number as u32, 0, Some("@all-tables"), true)?;
        for table in tables["tables"].as_array().into_iter().flatten() {
            let rows = table["cells"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|row| {
                    row.as_array()
                        .into_iter()
                        .flatten()
                        .map(markdown_cell)
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            render_pipe_table(&mut output, &rows);
        }
        if let Some(note) = notes_by_slide.get(&number)
            && !note.trim().is_empty()
        {
            output.push_str("<!-- Speaker notes:\n");
            output.push_str(&note.replace("--", "—"));
            output.push_str("\n-->\n\n");
        }
    }
    Ok(output)
}

fn render_json_runs(paragraph: &Value) -> String {
    let runs = paragraph["runs"].as_array();
    if let Some(runs) = runs {
        return runs
            .iter()
            .map(|run| {
                let mut text = markdown_escape(run["text"].as_str().unwrap_or_default());
                if run["bold"].as_bool().unwrap_or(false) && !text.is_empty() {
                    text = format!("**{text}**");
                }
                if run["italic"].as_bool().unwrap_or(false) && !text.is_empty() {
                    text = format!("*{text}*");
                }
                text
            })
            .collect();
    }
    markdown_escape(paragraph["text"].as_str().unwrap_or_default())
}

fn render_xlsx(value: &Value, formatted: bool) -> String {
    let values = value["values"].as_array().cloned().unwrap_or_default();
    let formats = value["numberFormatCodes"].as_array();
    let rows = values
        .iter()
        .enumerate()
        .map(|(row_index, row)| {
            row.as_array()
                .into_iter()
                .flatten()
                .enumerate()
                .map(|(col_index, cell)| {
                    let format = formats
                        .and_then(|rows| rows.get(row_index))
                        .and_then(Value::as_array)
                        .and_then(|row| row.get(col_index))
                        .and_then(Value::as_str);
                    markdown_xlsx_cell(cell, formatted.then_some(format).flatten())
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let mut output = String::new();
    render_pipe_table(&mut output, &rows);
    output
}

fn markdown_xlsx_cell(value: &Value, format: Option<&str>) -> String {
    let raw = if let Some(number) = value.as_f64() {
        format_xlsx_number(number, format)
    } else if let Some(value) = value.as_str() {
        if let (Ok(number), Some(format)) = (value.parse::<f64>(), format) {
            format_xlsx_number(number, Some(format))
        } else {
            value.to_string()
        }
    } else if let Some(value) = value.as_bool() {
        if value { "TRUE" } else { "FALSE" }.to_string()
    } else if value.is_null() {
        String::new()
    } else {
        value.to_string()
    };
    markdown_escape_cell(&raw)
}

fn format_xlsx_number(number: f64, format: Option<&str>) -> String {
    let Some(format) = format.filter(|format| !format.trim().is_empty()) else {
        return compact_number(number);
    };
    let section = format.split(';').next().unwrap_or(format);
    let lower = section.to_ascii_lowercase();
    if (lower.contains('y') || lower.contains('d')) && lower.contains('m') {
        let (year, month, day) = excel_serial_date(number.floor() as i64);
        if lower.contains("yyyy") {
            return format!("{year:04}-{month:02}-{day:02}");
        }
        return format!("{month}/{day}/{:02}", year.rem_euclid(100));
    }
    let decimals = section
        .split_once('.')
        .map(|(_, tail)| {
            tail.chars()
                .take_while(|ch| matches!(ch, '0' | '#'))
                .count()
        })
        .unwrap_or_default();
    if section.contains('%') {
        return format!("{:.*}%", decimals, number * 100.0);
    }
    let mut rendered = format!("{:.*}", decimals, number);
    if section.contains(',') {
        rendered = group_decimal(&rendered);
    }
    if section.contains('$') {
        rendered.insert(0, '$');
    }
    rendered
}

fn compact_number(number: f64) -> String {
    if number.fract() == 0.0 {
        format!("{number:.0}")
    } else {
        number.to_string()
    }
}

fn group_decimal(value: &str) -> String {
    let (integer, fraction) = value.split_once('.').unwrap_or((value, ""));
    let (sign, digits) = integer
        .strip_prefix('-')
        .map_or(("", integer), |digits| ("-", digits));
    let mut grouped = String::new();
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    if fraction.is_empty() {
        format!("{sign}{grouped}")
    } else {
        format!("{sign}{grouped}.{fraction}")
    }
}

fn excel_serial_date(serial: i64) -> (i64, i64, i64) {
    let z = serial - 25_569 + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (year, month, day)
}

fn render_outline(value: &Value) -> String {
    let family = value["type"]
        .as_str()
        .unwrap_or("package")
        .to_ascii_uppercase();
    let mut output = format!("# {family} outline\n\n");
    if let Some(file) = value["file"].as_str() {
        output.push_str(&format!("- File: `{}`\n", markdown_escape(file)));
    }
    if let Some(size) = value["fileSizeBytes"].as_u64() {
        output.push_str(&format!("- Size: {size} bytes\n"));
    }
    if let Some(hash) = value["documentHash"].as_str() {
        output.push_str(&format!("- Document hash: `{}`\n", markdown_escape(hash)));
    }
    output.push('\n');
    if let Some(summary) = value["summary"].as_object() {
        output.push_str("## Summary\n\n");
        let mut fields = summary.iter().collect::<Vec<_>>();
        fields.sort_by_key(|(name, _)| name.as_str());
        for (name, value) in fields {
            if value.is_number() || value.is_boolean() || value.is_string() {
                output.push_str(&format!("- {name}: {}\n", markdown_cell(value)));
            }
        }
        output.push('\n');
    }
    match value["type"].as_str() {
        Some("pptx") => {
            output.push_str("## Slides\n\n");
            for slide in value["slides"].as_array().into_iter().flatten() {
                let number = slide["number"].as_u64().unwrap_or_default();
                let title = slide["title"].as_str().unwrap_or("Untitled");
                output.push_str(&format!("### {number}. {}\n\n", markdown_escape(title)));
                outline_counts(&mut output, slide, &["shape", "chart", "table", "image"]);
            }
        }
        Some("xlsx") => {
            output.push_str("## Sheets\n\n");
            for sheet in value["sheets"].as_array().into_iter().flatten() {
                let name = sheet["name"].as_str().unwrap_or("Unnamed");
                output.push_str(&format!("### {}\n\n", markdown_escape(name)));
                if let Some(range) = sheet["usedRange"]["ref"].as_str() {
                    output.push_str(&format!("- Used range: `{}`\n", markdown_escape(range)));
                }
                outline_counts(
                    &mut output,
                    sheet,
                    &[
                        "cell",
                        "row",
                        "table",
                        "chart",
                        "pivot",
                        "validation",
                        "conditionalFormat",
                        "comment",
                    ],
                );
            }
        }
        Some("docx") => {
            output.push_str("## Blocks\n\n");
            for block in value["blocks"].as_array().into_iter().flatten() {
                let index = block["index"].as_u64().unwrap_or_default();
                let kind = block["kind"].as_str().unwrap_or("block");
                let preview = block["textPreview"].as_str().unwrap_or_default();
                output.push_str(&format!(
                    "- {index}. **{}**{}\n",
                    markdown_escape(kind),
                    if preview.is_empty() {
                        String::new()
                    } else {
                        format!(": {}", markdown_escape(preview))
                    }
                ));
            }
        }
        _ => {}
    }
    output
}

fn outline_counts(output: &mut String, value: &Value, kinds: &[&str]) {
    for kind in kinds {
        let key = format!("{kind}Count");
        if let Some(count) = value[&key].as_u64() {
            output.push_str(&format!("- {kind}s: {count}\n"));
        }
    }
    output.push('\n');
}

fn render_pipe_table(output: &mut String, rows: &[Vec<String>]) {
    let Some(header) = rows.first() else {
        return;
    };
    let columns = rows.iter().map(Vec::len).max().unwrap_or_default();
    if columns == 0 {
        return;
    }
    render_pipe_row(output, header, columns);
    output.push('|');
    for _ in 0..columns {
        output.push_str(" --- |");
    }
    output.push('\n');
    for row in rows.iter().skip(1) {
        render_pipe_row(output, row, columns);
    }
    output.push('\n');
}

fn render_pipe_row(output: &mut String, row: &[String], columns: usize) {
    output.push('|');
    for index in 0..columns {
        output.push(' ');
        output.push_str(row.get(index).map(String::as_str).unwrap_or_default());
        output.push_str(" |");
    }
    output.push('\n');
}

fn markdown_cell(value: &Value) -> String {
    if let Some(value) = value.as_str() {
        markdown_escape_cell(value)
    } else if value.is_null() {
        String::new()
    } else {
        markdown_escape_cell(&value.to_string())
    }
}

fn markdown_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('*', "\\*")
        .replace('_', "\\_")
        .replace('[', "\\[")
        .replace(']', "\\]")
}

fn markdown_escape_cell(value: &str) -> String {
    markdown_escape(value)
        .replace('|', "\\|")
        .replace(['\r', '\n'], "<br>")
}

fn markdown_url(value: &str) -> String {
    value
        .replace(' ', "%20")
        .replace('(', "\\(")
        .replace(')', "\\)")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spreadsheet_formats_cover_date_percent_and_currency() {
        assert_eq!(format_xlsx_number(45_292.0, Some("m/d/yy")), "1/1/24");
        assert_eq!(format_xlsx_number(0.125, Some("0.0%")), "12.5%");
        assert_eq!(format_xlsx_number(1234.5, Some("$#,##0.00")), "$1,234.50");
    }

    #[test]
    fn pipe_tables_escape_content_and_end_with_a_blank_line() {
        let mut output = String::new();
        render_pipe_table(
            &mut output,
            &[
                vec![markdown_escape_cell("A|B"), markdown_escape_cell("C")],
                vec![
                    markdown_escape_cell("line\nbreak"),
                    markdown_escape_cell("*literal*"),
                ],
            ],
        );
        assert_eq!(
            output,
            "| A\\|B | C |\n| --- | --- |\n| line<br>break | \\*literal\\* |\n\n"
        );
    }

    #[test]
    fn docx_numbering_maps_bullets_and_decimal_levels() {
        let formats = parse_docx_list_formats(
            r#"<w:numbering xmlns:w="urn:w"><w:abstractNum w:abstractNumId="7"><w:lvl w:ilvl="0"><w:numFmt w:val="bullet"/></w:lvl><w:lvl w:ilvl="1"><w:numFmt w:val="decimal"/></w:lvl></w:abstractNum><w:num w:numId="3"><w:abstractNumId w:val="7"/></w:num></w:numbering>"#,
        );
        assert_eq!(formats.get(&(3, 0)), Some(&true));
        assert_eq!(formats.get(&(3, 1)), Some(&false));
        assert_eq!(heading_level("Heading 3"), Some(3));
    }

    #[test]
    fn real_docx_and_pptx_content_maps_to_commonmark() {
        let docx = "testdata/docx/split-runs/document.docx";
        let docx_value = crate::docx_text(docx).expect("read DOCX fixture");
        let rendered = render_docx(docx, &docx_value).expect("render DOCX Markdown");
        assert!(rendered.contains("say **hello** again"), "{rendered}");

        let link_docx = "testdata/docx/hyperlink/document.docx";
        let link_value = crate::docx_text(link_docx).expect("read hyperlink fixture");
        let rendered = render_docx(link_docx, &link_value).expect("render link Markdown");
        assert!(
            rendered.contains("[link text](https://example.com)"),
            "{rendered}"
        );

        let pptx = "testdata/pptx/slide-assembly-notes-media/presentation.pptx";
        let rest = vec!["--slide".to_string(), "1".to_string()];
        let pptx_value = crate::pptx_extract_text(pptx, &rest).expect("read PPTX fixture");
        let rendered = render_pptx(pptx, &rest, &pptx_value).expect("render PPTX Markdown");
        assert!(
            rendered.starts_with("# Notes and Media Test\n"),
            "{rendered}"
        );
        assert!(rendered.contains("<!-- Speaker notes:\n"), "{rendered}");
    }

    #[test]
    fn real_xlsx_and_outline_content_maps_to_commonmark() {
        let xlsx = "testdata/xlsx/types-and-formulas/workbook.xlsx";
        let value = crate::xlsx_range_export_with_options(
            xlsx,
            "Types",
            "A1:H2",
            crate::XlsxRangeExportOptions {
                include_types: false,
                include_formulas: false,
                include_formats: true,
                data_out: None,
                max_cells: 0,
            },
        )
        .expect("read XLSX fixture");
        let rendered = render_xlsx(&value, true);
        assert!(rendered.starts_with("| Region | Revenue |"), "{rendered}");
        assert!(rendered.contains("| 1/1/24 |"), "{rendered}");

        let outline = crate::outline(
            xlsx,
            crate::OutlineOptions {
                depth: 2,
                text_preview: 80,
                slide: None,
                sheet: None,
                section: None,
            },
        )
        .expect("outline XLSX fixture");
        let rendered = render_outline(&outline);
        assert!(rendered.starts_with("# XLSX outline\n"), "{rendered}");
        assert!(rendered.contains("## Sheets\n"), "{rendered}");
        assert!(rendered.contains("- Used range: `A1:H4`"), "{rendered}");
    }
}
