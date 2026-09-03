use serde::Deserialize;
use std::fs;

use crate::{CliError, CliResult, needs_xml_space_preserve, xml_attr_escape, xml_escape};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ParagraphContext {
    Placeholder,
    Textbox,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ParagraphRun {
    #[serde(default)]
    pub(crate) text: String,
    pub(crate) bold: Option<bool>,
    pub(crate) italic: Option<bool>,
    pub(crate) size: Option<f64>,
    pub(crate) color: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Paragraph {
    #[serde(default)]
    pub(crate) text: String,
    #[serde(default)]
    pub(crate) level: u8,
    #[serde(default)]
    pub(crate) bullet: bool,
    pub(crate) bold: Option<bool>,
    pub(crate) italic: Option<bool>,
    pub(crate) size: Option<f64>,
    pub(crate) color: Option<String>,
    pub(crate) align: Option<String>,
    #[serde(skip)]
    pub(crate) font_family: Option<String>,
    #[serde(default)]
    pub(crate) runs: Vec<ParagraphRun>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ParagraphDefaults {
    pub(crate) level: u8,
    pub(crate) bold: Option<bool>,
    pub(crate) italic: Option<bool>,
    pub(crate) size: Option<f64>,
    pub(crate) color: Option<String>,
    pub(crate) align: Option<String>,
    pub(crate) font_family: Option<String>,
}

pub(crate) fn paragraphs_from_text(
    text: &str,
    defaults: &ParagraphDefaults,
) -> CliResult<Vec<Paragraph>> {
    text.split('\n')
        .map(|line| paragraph_from_line(line.strip_suffix('\r').unwrap_or(line), defaults))
        .collect()
}

pub(crate) fn paragraphs_from_file(
    path: &str,
    defaults: &ParagraphDefaults,
) -> CliResult<Vec<Paragraph>> {
    if path.trim().is_empty() {
        return Err(CliError::invalid_args(
            "--paragraphs-file path cannot be empty",
        ));
    }
    let contents = fs::read_to_string(path)
        .map_err(|err| CliError::file_not_found(format!("cannot read {path}: {err}")))?;
    let mut paragraphs: Vec<Paragraph> = serde_json::from_str(&contents).map_err(|err| {
        CliError::invalid_args(format!("invalid paragraphs JSON in {path}: {err}"))
    })?;
    if paragraphs.is_empty() {
        return Err(CliError::invalid_args(format!(
            "paragraphs JSON in {path} must contain at least one paragraph"
        )));
    }
    for paragraph in &mut paragraphs {
        apply_defaults(paragraph, defaults);
        validate_paragraph(paragraph)?;
    }
    Ok(paragraphs)
}

pub(crate) fn text_body_xml(
    paragraphs: &[Paragraph],
    context: ParagraphContext,
    body_properties: &str,
) -> CliResult<String> {
    Ok(format!(
        "<p:txBody>{}<a:lstStyle/>{}</p:txBody>",
        body_properties,
        render_paragraphs(paragraphs, context)?
    ))
}

pub(crate) fn render_paragraphs(
    paragraphs: &[Paragraph],
    context: ParagraphContext,
) -> CliResult<String> {
    let mut xml = String::new();
    let fallback = Paragraph::default();
    let paragraphs: &[Paragraph] = if paragraphs.is_empty() {
        std::slice::from_ref(&fallback)
    } else {
        paragraphs
    };
    for paragraph in paragraphs {
        validate_paragraph(paragraph)?;
        xml.push_str("<a:p>");
        xml.push_str(&paragraph_properties_xml(paragraph, context));
        if paragraph.runs.is_empty() {
            xml.push_str(&run_xml(
                &ParagraphRun {
                    text: paragraph.text.clone(),
                    bold: paragraph.bold,
                    italic: paragraph.italic,
                    size: paragraph.size,
                    color: paragraph.color.clone(),
                },
                paragraph,
            ));
        } else {
            for run in &paragraph.runs {
                xml.push_str(&run_xml(run, paragraph));
            }
        }
        xml.push_str("<a:endParaRPr lang=\"en-US\"/></a:p>");
    }
    Ok(xml)
}

fn paragraph_from_line(line: &str, defaults: &ParagraphDefaults) -> CliResult<Paragraph> {
    let tab_count = line.chars().take_while(|ch| *ch == '\t').count();
    let remaining = &line[tab_count..];
    let (bullet, text) = remaining
        .strip_prefix("- ")
        .or_else(|| remaining.strip_prefix("* "))
        .map_or((false, remaining), |text| (true, text));
    let level = defaults
        .level
        .checked_add(u8::try_from(tab_count).unwrap_or(u8::MAX))
        .ok_or_else(|| CliError::invalid_args("paragraph level exceeds 8"))?;
    let mut paragraph = Paragraph {
        text: text.to_string(),
        level,
        bullet,
        bold: defaults.bold,
        italic: defaults.italic,
        size: defaults.size,
        color: defaults.color.clone(),
        align: defaults.align.clone(),
        font_family: defaults.font_family.clone(),
        runs: Vec::new(),
    };
    apply_defaults(&mut paragraph, defaults);
    validate_paragraph(&paragraph)?;
    Ok(paragraph)
}

fn apply_defaults(paragraph: &mut Paragraph, defaults: &ParagraphDefaults) {
    if paragraph.level == 0 {
        paragraph.level = defaults.level;
    }
    paragraph.bold = paragraph.bold.or(defaults.bold);
    paragraph.italic = paragraph.italic.or(defaults.italic);
    paragraph.size = paragraph.size.or(defaults.size);
    if paragraph.color.is_none() {
        paragraph.color.clone_from(&defaults.color);
    }
    if paragraph.align.is_none() {
        paragraph.align.clone_from(&defaults.align);
    }
    if paragraph.font_family.is_none() {
        paragraph.font_family.clone_from(&defaults.font_family);
    }
}

fn validate_paragraph(paragraph: &Paragraph) -> CliResult<()> {
    if paragraph.level > 8 {
        return Err(CliError::invalid_args(format!(
            "paragraph level {} is out of range [0, 8]",
            paragraph.level
        )));
    }
    validate_size_color(paragraph.size, paragraph.color.as_deref())?;
    if let Some(align) = paragraph.align.as_deref() {
        normalize_align(align)?;
    }
    for run in &paragraph.runs {
        validate_size_color(run.size, run.color.as_deref())?;
    }
    Ok(())
}

fn validate_size_color(size: Option<f64>, color: Option<&str>) -> CliResult<()> {
    if size.is_some_and(|size| !size.is_finite() || size <= 0.0) {
        return Err(CliError::invalid_args(
            "paragraph size must be greater than 0",
        ));
    }
    if let Some(color) = color
        && (color.len() != 6 || !color.bytes().all(|byte| byte.is_ascii_hexdigit()))
    {
        return Err(CliError::invalid_args(format!(
            "invalid paragraph color {color:?} (expected 6 hex digits)"
        )));
    }
    Ok(())
}

fn paragraph_properties_xml(paragraph: &Paragraph, context: ParagraphContext) -> String {
    let mut xml = String::from("<a:pPr");
    if paragraph.level > 0 {
        xml.push_str(&format!(r#" lvl="{}""#, paragraph.level));
    }
    if let Some(align) = paragraph.align.as_deref() {
        xml.push_str(&format!(
            r#" algn="{}""#,
            normalize_align(align).unwrap_or(align)
        ));
    }
    match (paragraph.bullet, context) {
        (true, ParagraphContext::Textbox) => xml.push_str("><a:buChar char=\"•\"/></a:pPr>"),
        (true, ParagraphContext::Placeholder) => xml.push_str("/>"),
        (false, _) => xml.push_str("><a:buNone/></a:pPr>"),
    }
    xml
}

fn run_xml(run: &ParagraphRun, paragraph: &Paragraph) -> String {
    let bold = run.bold.or(paragraph.bold);
    let italic = run.italic.or(paragraph.italic);
    let size = run.size.or(paragraph.size);
    let color = run.color.as_deref().or(paragraph.color.as_deref());
    let font_family = paragraph.font_family.as_deref();
    let has_properties = bold.is_some()
        || italic.is_some()
        || size.is_some()
        || color.is_some()
        || font_family.is_some();
    let mut xml = String::from("<a:r>");
    if has_properties {
        xml.push_str("<a:rPr lang=\"en-US\"");
        if let Some(bold) = bold {
            xml.push_str(if bold { " b=\"1\"" } else { " b=\"0\"" });
        }
        if let Some(italic) = italic {
            xml.push_str(if italic { " i=\"1\"" } else { " i=\"0\"" });
        }
        if let Some(size) = size {
            xml.push_str(&format!(" sz=\"{}\"", (size * 100.0).round() as i64));
        }
        if color.is_none() && font_family.is_none() {
            xml.push_str("/>");
        } else {
            xml.push('>');
            if let Some(color) = color {
                xml.push_str(&format!(
                    "<a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill>",
                    xml_attr_escape(color)
                ));
            }
            if let Some(font_family) = font_family {
                xml.push_str(&format!(
                    "<a:latin typeface=\"{}\"/>",
                    xml_attr_escape(font_family)
                ));
            }
            xml.push_str("</a:rPr>");
        }
    }
    if needs_xml_space_preserve(&run.text) {
        xml.push_str(&format!(
            "<a:t xml:space=\"preserve\">{}</a:t>",
            xml_escape(&run.text)
        ));
    } else {
        xml.push_str(&format!("<a:t>{}</a:t>", xml_escape(&run.text)));
    }
    xml.push_str("</a:r>");
    xml
}

fn normalize_align(value: &str) -> CliResult<&'static str> {
    match value {
        "left" | "l" => Ok("l"),
        "center" | "centre" | "ctr" => Ok("ctr"),
        "right" | "r" => Ok("r"),
        "justify" | "just" => Ok("just"),
        "distributed" | "dist" => Ok("dist"),
        _ => Err(CliError::invalid_args(format!(
            "invalid paragraph alignment {value:?}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plaintext_markers_create_independent_paragraphs() {
        let paragraphs =
            paragraphs_from_text("- One\n\t* Nested\nPlain", &ParagraphDefaults::default())
                .expect("parse paragraphs");
        assert_eq!(paragraphs.len(), 3);
        assert!(paragraphs[0].bullet);
        assert_eq!(paragraphs[1].level, 1);
        assert!(paragraphs[1].bullet);
        assert_eq!(paragraphs[2].text, "Plain");
    }

    #[test]
    fn placeholder_and_textbox_bullets_have_distinct_xml() {
        let paragraph = Paragraph {
            text: "Item".to_string(),
            bullet: true,
            ..Paragraph::default()
        };
        let placeholder = render_paragraphs(
            std::slice::from_ref(&paragraph),
            ParagraphContext::Placeholder,
        )
        .expect("placeholder XML");
        let textbox =
            render_paragraphs(&[paragraph], ParagraphContext::Textbox).expect("textbox XML");
        assert!(!placeholder.contains("buChar"));
        assert!(textbox.contains("<a:buChar char=\"•\"/>"));
    }
}
