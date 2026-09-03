//! Deterministic text measurement from committed numeric font metrics.
//!
//! The tables contain advances only; no font programs are embedded. Metrics
//! are normalized to 1,000 units per em so the estimator does not depend on a
//! platform font API at runtime.

mod pptx;

pub(crate) use pptx::pptx_text_measure;

use std::collections::BTreeMap;
use std::sync::OnceLock;

const EMU_PER_POINT: f64 = 12_700.0;
const FIRST_PRINTABLE: u32 = 0x20;
const LAST_PRINTABLE: u32 = 0x7e;
const PRINTABLE_COUNT: usize = (LAST_PRINTABLE - FIRST_PRINTABLE + 1) as usize;
const FIRST_LATIN1: u32 = 0xa0;
const LAST_LATIN1: u32 = 0xff;
const LATIN1_COUNT: usize = (LAST_LATIN1 - FIRST_LATIN1 + 1) as usize;
const PROFILE_DATA: &str = include_str!("../testdata/fonts/metrics-v1.tsv");
const LATIN1_DATA: &str = include_str!("../testdata/fonts/latin1-v1.tsv");
const FAMILY_DATA: &str = include_str!("../testdata/fonts/families-v1.tsv");

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontMetrics {
    pub family: String,
    pub source_family: String,
    pub selection: String,
    pub average_advance_regular: u16,
    pub average_advance_bold: u16,
    pub line_height: u16,
    regular_advances: Vec<u16>,
    bold_advances: Vec<u16>,
    regular_latin1_advances: Vec<u16>,
    bold_latin1_advances: Vec<u16>,
}

impl FontMetrics {
    pub fn average_advance(&self, bold: bool) -> u16 {
        if bold {
            self.average_advance_bold
        } else {
            self.average_advance_regular
        }
    }

    pub fn advance(&self, character: char, bold: bool) -> u16 {
        if character == '\t' {
            return self.advance(' ', bold).saturating_mul(4);
        }
        if is_combining_mark(character) {
            return 0;
        }
        let codepoint = u32::from(character);
        if (FIRST_PRINTABLE..=LAST_PRINTABLE).contains(&codepoint) {
            let index = (codepoint - FIRST_PRINTABLE) as usize;
            return if bold {
                self.bold_advances[index]
            } else {
                self.regular_advances[index]
            };
        }
        if (FIRST_LATIN1..=LAST_LATIN1).contains(&codepoint) {
            let index = (codepoint - FIRST_LATIN1) as usize;
            return if bold {
                self.bold_latin1_advances[index]
            } else {
                self.regular_latin1_advances[index]
            };
        }
        if character.is_whitespace() {
            return self.advance(' ', bold);
        }
        if is_wide_character(character) {
            return 1_000;
        }
        self.average_advance(bold)
    }

    pub fn printable_advances(&self, bold: bool) -> &[u16] {
        if bold {
            &self.bold_advances
        } else {
            &self.regular_advances
        }
    }

    pub fn latin1_advances(&self, bold: bool) -> &[u16] {
        if bold {
            &self.bold_latin1_advances
        } else {
            &self.regular_latin1_advances
        }
    }

    pub fn fallback_warning(&self, requested_family: &str) -> Option<String> {
        (self.family == "*").then(|| {
            format!(
                "Unknown font family {requested_family:?}; using fallback metrics from {}.",
                self.source_family
            )
        })
    }
}

#[derive(Clone, Debug)]
pub struct ParagraphMeasure<'a> {
    pub text: &'a str,
    pub font_family: &'a str,
    pub font_size_points: f64,
    pub bold: bool,
    pub left_indent_emu: i64,
    pub right_indent_emu: i64,
    pub first_line_indent_emu: i64,
    pub bullet: bool,
    /// Multiplier applied to the font's measured line height.
    pub line_spacing: f64,
}

impl<'a> ParagraphMeasure<'a> {
    pub fn plain(text: &'a str, font_family: &'a str, font_size_points: f64) -> Self {
        Self {
            text,
            font_family,
            font_size_points,
            bold: false,
            left_indent_emu: 0,
            right_indent_emu: 0,
            first_line_indent_emu: 0,
            bullet: false,
            line_spacing: 1.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParagraphMeasurement {
    pub line_count: usize,
    pub height_emu: i64,
    pub line_height_emu: i64,
    pub max_line_width_emu: i64,
    pub unwrapped_width_emu: i64,
    pub available_width_emu: i64,
    pub font_family: String,
    pub source_font_family: String,
    pub metric_selection: String,
    pub warning: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextBoxMeasurement {
    pub line_count: usize,
    pub height_emu: i64,
    pub available_width_emu: i64,
    pub available_height_emu: i64,
    pub overflows_vertically: bool,
    pub autofit_mode: &'static str,
    pub effective_font_scale: f64,
    pub paragraphs: Vec<ParagraphMeasurement>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum TextAutofit {
    #[default]
    None,
    ResizeShape,
    ShrinkText {
        font_scale: f64,
        line_spacing_reduction: f64,
    },
}

impl TextAutofit {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::ResizeShape => "resize-shape",
            Self::ShrinkText { .. } => "shrink-text",
        }
    }
}

#[derive(Clone)]
struct MetricProfile {
    source_family: String,
    average_regular: u16,
    average_bold: u16,
    line_height: u16,
    regular_advances: Vec<u16>,
    bold_advances: Vec<u16>,
    regular_latin1_advances: Vec<u16>,
    bold_latin1_advances: Vec<u16>,
}

struct MetricDatabase {
    fonts: Vec<FontMetrics>,
    fallback: usize,
}

static METRICS: OnceLock<MetricDatabase> = OnceLock::new();

pub fn font_metrics(family: &str) -> &'static FontMetrics {
    let database = METRICS.get_or_init(|| {
        parse_metric_database().expect("committed font metric tables must be internally valid")
    });
    let requested = family
        .trim()
        .trim_matches(|character| matches!(character, '\'' | '"'))
        .split(',')
        .next()
        .unwrap_or_default()
        .trim();
    database
        .fonts
        .iter()
        .find(|metrics| metrics.family.eq_ignore_ascii_case(requested))
        .or_else(|| {
            database.fonts.iter().find(|metrics| {
                requested.len() > metrics.family.len()
                    && requested
                        .get(..metrics.family.len())
                        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(&metrics.family))
            })
        })
        .unwrap_or(&database.fonts[database.fallback])
}

pub fn supported_font_families() -> Vec<&'static str> {
    let database = METRICS.get_or_init(|| {
        parse_metric_database().expect("committed font metric tables must be internally valid")
    });
    database
        .fonts
        .iter()
        .filter(|metrics| metrics.family != "*")
        .map(|metrics| metrics.family.as_str())
        .collect()
}

pub fn measure_text_width_emu(
    text: &str,
    font_family: &str,
    font_size_points: f64,
    bold: bool,
) -> i64 {
    let metrics = font_metrics(font_family);
    units_to_emu(
        text.chars()
            .map(|character| u64::from(metrics.advance(character, bold)))
            .sum(),
        font_size_points,
    )
}

pub fn measure_paragraph(
    paragraph: &ParagraphMeasure<'_>,
    box_width_emu: i64,
) -> ParagraphMeasurement {
    let metrics = font_metrics(paragraph.font_family);
    let font_size = finite_positive_or(paragraph.font_size_points, 18.0);
    let line_spacing = finite_positive_or(paragraph.line_spacing, 1.0);
    let base_width = box_width_emu
        .saturating_sub(paragraph.left_indent_emu.max(0))
        .saturating_sub(paragraph.right_indent_emu.max(0))
        .max(1);
    let first_width = box_width_emu
        .saturating_sub(
            paragraph
                .left_indent_emu
                .saturating_add(paragraph.first_line_indent_emu)
                .max(0),
        )
        .saturating_sub(paragraph.right_indent_emu.max(0))
        .saturating_sub(if paragraph.bullet {
            units_to_emu(900, font_size)
        } else {
            0
        })
        .max(1);
    let wrapped = wrapped_line_widths(
        paragraph.text,
        metrics,
        font_size,
        paragraph.bold,
        first_width,
        base_width,
    );
    let line_height_emu =
        ((f64::from(metrics.line_height) / 1_000.0) * font_size * EMU_PER_POINT * line_spacing)
            .round() as i64;
    let unwrapped_width_emu = paragraph
        .text
        .split('\n')
        .map(|line| measure_text_width_emu(line, paragraph.font_family, font_size, paragraph.bold))
        .max()
        .unwrap_or_default();
    ParagraphMeasurement {
        line_count: wrapped.len(),
        height_emu: line_height_emu.saturating_mul(wrapped.len() as i64),
        line_height_emu,
        max_line_width_emu: wrapped.into_iter().max().unwrap_or_default(),
        unwrapped_width_emu,
        available_width_emu: base_width,
        font_family: metrics.family.clone(),
        source_font_family: metrics.source_family.clone(),
        metric_selection: metrics.selection.clone(),
        warning: metrics.fallback_warning(paragraph.font_family),
    }
}

pub fn measure_text_box(
    paragraphs: &[ParagraphMeasure<'_>],
    box_width_emu: i64,
    box_height_emu: i64,
    left_inset_emu: i64,
    right_inset_emu: i64,
    top_inset_emu: i64,
    bottom_inset_emu: i64,
) -> TextBoxMeasurement {
    measure_text_box_with_autofit(
        paragraphs,
        box_width_emu,
        box_height_emu,
        left_inset_emu,
        right_inset_emu,
        top_inset_emu,
        bottom_inset_emu,
        TextAutofit::None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn measure_text_box_with_autofit(
    paragraphs: &[ParagraphMeasure<'_>],
    box_width_emu: i64,
    box_height_emu: i64,
    left_inset_emu: i64,
    right_inset_emu: i64,
    top_inset_emu: i64,
    bottom_inset_emu: i64,
    autofit: TextAutofit,
) -> TextBoxMeasurement {
    let available_width = box_width_emu
        .saturating_sub(left_inset_emu.max(0))
        .saturating_sub(right_inset_emu.max(0))
        .max(1);
    let available_height = box_height_emu
        .saturating_sub(top_inset_emu.max(0))
        .saturating_sub(bottom_inset_emu.max(0))
        .max(0);
    let (font_scale, line_spacing_scale) = match autofit {
        TextAutofit::ShrinkText {
            font_scale,
            line_spacing_reduction,
        } => (
            finite_ratio_or(font_scale, 1.0),
            1.0 - finite_ratio_or(line_spacing_reduction, 0.0).clamp(0.0, 0.95),
        ),
        TextAutofit::None | TextAutofit::ResizeShape => (1.0, 1.0),
    };
    let measurements = paragraphs
        .iter()
        .map(|paragraph| {
            let adjusted = ParagraphMeasure {
                text: paragraph.text,
                font_family: paragraph.font_family,
                font_size_points: paragraph.font_size_points * font_scale,
                bold: paragraph.bold,
                left_indent_emu: paragraph.left_indent_emu,
                right_indent_emu: paragraph.right_indent_emu,
                first_line_indent_emu: paragraph.first_line_indent_emu,
                bullet: paragraph.bullet,
                line_spacing: paragraph.line_spacing * line_spacing_scale,
            };
            measure_paragraph(&adjusted, available_width)
        })
        .collect::<Vec<_>>();
    let line_count = measurements.iter().map(|item| item.line_count).sum();
    let height_emu = measurements.iter().map(|item| item.height_emu).sum();
    TextBoxMeasurement {
        line_count,
        height_emu,
        available_width_emu: available_width,
        available_height_emu: available_height,
        overflows_vertically: height_emu > available_height,
        autofit_mode: autofit.as_str(),
        effective_font_scale: font_scale,
        paragraphs: measurements,
    }
}

/// Estimate an Excel column width in the character-width units stored by XLSX.
/// The 5 px term is Excel's conventional cell padding allowance.
pub fn estimate_excel_column_width(
    text: &str,
    font_family: &str,
    font_size_points: f64,
    bold: bool,
    number_format_code: Option<&str>,
) -> f64 {
    let metrics = font_metrics(font_family);
    let size = finite_positive_or(font_size_points, 11.0);
    let text_units = text
        .chars()
        .map(|character| u64::from(metrics.advance(character, bold)))
        .sum::<u64>() as f64;
    let format_units = number_format_allowance(number_format_code, metrics.average_advance(bold));
    let text_pixels = (text_units + format_units) / 1_000.0 * size * 96.0 / 72.0;
    let digit_pixels = f64::from(metrics.advance('0', bold)) / 1_000.0 * size * 96.0 / 72.0;
    ((text_pixels + 5.0) / digit_pixels.max(1.0)).max(0.0)
}

fn wrapped_line_widths(
    text: &str,
    metrics: &FontMetrics,
    font_size_points: f64,
    bold: bool,
    first_width_emu: i64,
    following_width_emu: i64,
) -> Vec<i64> {
    let mut all_lines = Vec::new();
    let mut first_output_line = true;
    for explicit_line in text.split('\n') {
        let first_limit = if first_output_line {
            first_width_emu
        } else {
            following_width_emu
        };
        let mut lines = wrap_explicit_line(
            explicit_line,
            metrics,
            font_size_points,
            bold,
            first_limit,
            following_width_emu,
        );
        first_output_line = false;
        all_lines.append(&mut lines);
    }
    if all_lines.is_empty() {
        all_lines.push(0);
    }
    all_lines
}

fn wrap_explicit_line(
    text: &str,
    metrics: &FontMetrics,
    font_size_points: f64,
    bold: bool,
    first_limit: i64,
    following_limit: i64,
) -> Vec<i64> {
    if text.is_empty() {
        return vec![0];
    }
    let space = units_to_emu(u64::from(metrics.advance(' ', bold)), font_size_points);
    let mut widths = Vec::new();
    let mut current = 0_i64;
    for word in text.split_whitespace() {
        let word_width = units_to_emu(
            word.chars()
                .map(|character| u64::from(metrics.advance(character, bold)))
                .sum(),
            font_size_points,
        );
        let separator = if current == 0 { 0 } else { space };
        let limit = if widths.is_empty() {
            first_limit
        } else {
            following_limit
        };
        if current > 0 && current.saturating_add(separator).saturating_add(word_width) > limit {
            widths.push(current);
            current = 0;
        }
        let mut limit = if widths.is_empty() {
            first_limit
        } else {
            following_limit
        };
        if current == 0 && word_width > limit {
            for character in word.chars() {
                let character_width = units_to_emu(
                    u64::from(metrics.advance(character, bold)),
                    font_size_points,
                );
                if current > 0 && current.saturating_add(character_width) > limit {
                    widths.push(current);
                    current = 0;
                    limit = following_limit;
                }
                current = current.saturating_add(character_width);
            }
        } else {
            current = current.saturating_add(if current == 0 { 0 } else { separator });
            current = current.saturating_add(word_width);
        }
    }
    widths.push(current);
    widths
}

fn units_to_emu(units: u64, font_size_points: f64) -> i64 {
    ((units as f64 / 1_000.0) * finite_positive_or(font_size_points, 1.0) * EMU_PER_POINT).round()
        as i64
}

fn number_format_allowance(code: Option<&str>, average: u16) -> f64 {
    let Some(code) = code else {
        return 0.0;
    };
    let lower = code.to_ascii_lowercase();
    let characters = if lower.contains('%') {
        1.0
    } else if lower.contains('$')
        || lower.contains('€')
        || lower.contains('£')
        || lower.contains("yy")
        || lower.contains("mm")
        || lower.contains("dd")
    {
        2.0
    } else if lower.contains('e') && lower.contains('+') {
        4.0
    } else {
        0.0
    };
    characters * f64::from(average)
}

fn finite_positive_or(value: f64, fallback: f64) -> f64 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        fallback
    }
}

fn finite_ratio_or(value: f64, fallback: f64) -> f64 {
    if value.is_finite() && value >= 0.0 {
        value
    } else {
        fallback
    }
}

fn is_combining_mark(character: char) -> bool {
    matches!(u32::from(character), 0x0300..=0x036f | 0x1ab0..=0x1aff | 0x1dc0..=0x1dff | 0xfe20..=0xfe2f)
}

fn is_wide_character(character: char) -> bool {
    matches!(u32::from(character), 0x1100..=0x115f | 0x2e80..=0xa4cf | 0xac00..=0xd7a3 | 0xf900..=0xfaff | 0x1f300..=0x1faff | 0x20000..=0x3fffd)
}

fn parse_metric_database() -> Result<MetricDatabase, String> {
    let mut profiles = BTreeMap::<String, MetricProfile>::new();
    for (index, line) in PROFILE_DATA.lines().enumerate() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let fields = line.split('\t').collect::<Vec<_>>();
        if fields.len() != 7 {
            return Err(format!(
                "metrics-v1.tsv line {} has {} fields",
                index + 1,
                fields.len()
            ));
        }
        let parse_number = |field: &str, name: &str| {
            field.parse::<u16>().map_err(|error| {
                format!("metrics-v1.tsv line {} invalid {name}: {error}", index + 1)
            })
        };
        let regular_advances = parse_advances(fields[5], index + 1, "regular")?;
        let bold_advances = parse_advances(fields[6], index + 1, "bold")?;
        profiles.insert(
            fields[0].to_string(),
            MetricProfile {
                source_family: fields[1].to_string(),
                average_regular: parse_number(fields[2], "regular average")?,
                average_bold: parse_number(fields[3], "bold average")?,
                line_height: parse_number(fields[4], "line height")?,
                regular_advances,
                bold_advances,
                regular_latin1_advances: Vec::new(),
                bold_latin1_advances: Vec::new(),
            },
        );
    }
    for (index, line) in LATIN1_DATA.lines().enumerate() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let fields = line.split('\t').collect::<Vec<_>>();
        if fields.len() != 3 {
            return Err(format!(
                "latin1-v1.tsv line {} has {} fields",
                index + 1,
                fields.len()
            ));
        }
        let profile = profiles.get_mut(fields[0]).ok_or_else(|| {
            format!(
                "latin1-v1.tsv line {} names missing profile {}",
                index + 1,
                fields[0]
            )
        })?;
        if !profile.regular_latin1_advances.is_empty() {
            return Err(format!(
                "latin1-v1.tsv line {} duplicates profile {}",
                index + 1,
                fields[0]
            ));
        }
        profile.regular_latin1_advances = parse_latin1_advances(fields[1], index + 1, "regular")?;
        profile.bold_latin1_advances = parse_latin1_advances(fields[2], index + 1, "bold")?;
    }
    if let Some((name, _)) = profiles
        .iter()
        .find(|(_, profile)| profile.regular_latin1_advances.len() != LATIN1_COUNT)
    {
        return Err(format!("latin1-v1.tsv is missing profile {name}"));
    }
    let mut fonts = Vec::new();
    let mut fallback = None;
    for (index, line) in FAMILY_DATA.lines().enumerate() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let fields = line.split('\t').collect::<Vec<_>>();
        if fields.len() != 3 {
            return Err(format!(
                "families-v1.tsv line {} has {} fields",
                index + 1,
                fields.len()
            ));
        }
        let profile = profiles.get(fields[1]).ok_or_else(|| {
            format!(
                "families-v1.tsv line {} names missing profile {}",
                index + 1,
                fields[1]
            )
        })?;
        if fields[0] == "*" {
            fallback = Some(fonts.len());
        }
        fonts.push(FontMetrics {
            family: fields[0].to_string(),
            source_family: profile.source_family.clone(),
            selection: fields[2].to_string(),
            average_advance_regular: profile.average_regular,
            average_advance_bold: profile.average_bold,
            line_height: profile.line_height,
            regular_advances: profile.regular_advances.clone(),
            bold_advances: profile.bold_advances.clone(),
            regular_latin1_advances: profile.regular_latin1_advances.clone(),
            bold_latin1_advances: profile.bold_latin1_advances.clone(),
        });
    }
    let fallback =
        fallback.ok_or_else(|| "families-v1.tsv is missing the * fallback".to_string())?;
    Ok(MetricDatabase { fonts, fallback })
}

fn parse_advances(value: &str, line: usize, weight: &str) -> Result<Vec<u16>, String> {
    let advances = value
        .split(',')
        .map(|item| {
            item.parse::<u16>().map_err(|error| {
                format!("metrics-v1.tsv line {line} invalid {weight} advance: {error}")
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if advances.len() != PRINTABLE_COUNT {
        return Err(format!(
            "metrics-v1.tsv line {line} has {} {weight} advances; expected {PRINTABLE_COUNT}",
            advances.len()
        ));
    }
    Ok(advances)
}

fn parse_latin1_advances(value: &str, line: usize, weight: &str) -> Result<Vec<u16>, String> {
    let advances = value
        .split(',')
        .map(|item| {
            item.parse::<u16>().map_err(|error| {
                format!("latin1-v1.tsv line {line} invalid {weight} advance: {error}")
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if advances.len() != LATIN1_COUNT {
        return Err(format!(
            "latin1-v1.tsv line {line} has {} {weight} advances; expected {LATIN1_COUNT}",
            advances.len()
        ));
    }
    Ok(advances)
}
