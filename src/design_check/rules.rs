use serde::Serialize;

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RuleDefinition {
    pub(super) code: &'static str,
    pub(super) family: &'static str,
    pub(super) severity: &'static str,
    pub(super) description: &'static str,
}

pub(super) const RULES: &[RuleDefinition] = &[
    rule(
        "PPTX_TEXT_CONTRAST",
        "pptx",
        "error",
        "Text contrast is below WCAG AA for its resolved foreground and background.",
    ),
    rule(
        "PPTX_FONT_TOO_SMALL",
        "pptx",
        "warning",
        "Text uses a font size below 12 points.",
    ),
    rule(
        "PPTX_BULLET_OVERLOAD",
        "pptx",
        "warning",
        "A placeholder has more than seven bullets or uses more than two list levels.",
    ),
    rule(
        "PPTX_FONT_OUTSIDE_THEME",
        "pptx",
        "warning",
        "A slide uses more than one font family outside the theme fonts.",
    ),
    rule(
        "PPTX_EMPTY_PLACEHOLDER",
        "pptx",
        "warning",
        "An empty placeholder remains on a slide.",
    ),
    rule(
        "PPTX_IMAGE_SCALE",
        "pptx",
        "warning",
        "An image is distorted or scaled beyond its native resolution.",
    ),
    rule(
        "PPTX_MISSING_TITLE",
        "pptx",
        "error",
        "A slide has no non-empty title.",
    ),
    rule(
        "PPTX_MISSING_ALT_TEXT",
        "pptx",
        "error",
        "An image has no alternative text.",
    ),
    rule(
        "PPTX_OUTSIDE_SAFE_MARGIN",
        "pptx",
        "warning",
        "Content enters the slide safe-margin area.",
    ),
    rule(
        "PPTX_INCONSISTENT_TITLE_POSITION",
        "pptx",
        "warning",
        "Title positions differ across slides that use the same layout.",
    ),
    rule(
        "DOCX_DANGLING_STYLE",
        "docx",
        "error",
        "Content references a style that is not defined in the package.",
    ),
    rule(
        "DOCX_HEADING_LEVEL_SKIP",
        "docx",
        "warning",
        "Adjacent headings skip a hierarchy level.",
    ),
    rule(
        "DOCX_EXCESS_EMPTY_PARAGRAPHS",
        "docx",
        "warning",
        "More than three consecutive empty paragraphs create accidental whitespace.",
    ),
    rule(
        "DOCX_TABLE_TOO_WIDE",
        "docx",
        "error",
        "A table is wider than the section text area.",
    ),
    rule(
        "DOCX_IMAGE_TOO_WIDE",
        "docx",
        "error",
        "An image is wider than the section text area.",
    ),
    rule(
        "DOCX_MISSING_ALT_TEXT",
        "docx",
        "error",
        "An image has no alternative text.",
    ),
    rule(
        "DOCX_REDUNDANT_DIRECT_FORMATTING",
        "docx",
        "warning",
        "Direct formatting duplicates the applied paragraph style.",
    ),
    rule(
        "DOCX_FONT_OUTSIDE_THEME",
        "docx",
        "warning",
        "Content uses a font family outside the document theme.",
    ),
    rule(
        "XLSX_NUMBER_CLIPPED",
        "xlsx",
        "error",
        "A numeric value is likely to render as #### at the current column width.",
    ),
    rule(
        "XLSX_HEADER_NOT_FROZEN",
        "xlsx",
        "warning",
        "A data sheet over 30 rows does not freeze its header row.",
    ),
    rule(
        "XLSX_INCONSISTENT_NUMBER_FORMAT",
        "xlsx",
        "warning",
        "A column mixes number formats for populated data cells.",
    ),
    rule(
        "XLSX_MULTIPLE_FONTS",
        "xlsx",
        "warning",
        "A worksheet uses more than one font family.",
    ),
    rule(
        "XLSX_MISSING_TABLE",
        "xlsx",
        "warning",
        "A tabular range over 100 rows is not represented by an Excel table.",
    ),
    rule(
        "XLSX_CHART_MISSING_TITLE",
        "xlsx",
        "error",
        "A chart has no title.",
    ),
    rule(
        "XLSX_UNREADABLE_TAB_COUNT",
        "xlsx",
        "warning",
        "The workbook has more visible sheet tabs than can be scanned comfortably.",
    ),
];

const fn rule(
    code: &'static str,
    family: &'static str,
    severity: &'static str,
    description: &'static str,
) -> RuleDefinition {
    RuleDefinition {
        code,
        family,
        severity,
        description,
    }
}

pub(super) fn definition(code: &str) -> Option<&'static RuleDefinition> {
    RULES.iter().find(|rule| rule.code == code)
}
