use serde_json::{Value, json};

pub(crate) const CAPABILITY_OBJECT_KINDS: &[&str] = &[
    "package",
    "template",
    "slide",
    "shape",
    "animation",
    "master",
    "layout",
    "placeholder",
    "sheet",
    "range",
    "conditional-format",
    "data-validation",
    "cell",
    "hyperlink",
    "table",
    "pivot",
    "name",
    "block",
    "paragraph",
    "style",
    "theme",
    "comment",
    "chart",
    "field",
    "header",
    "footer",
    "image",
    "media",
    "module",
];

pub(crate) const CAPABILITY_COMMAND_FAMILY_FILTERS: &[&str] = &[
    "pptx",
    "xlsx",
    "docx",
    "vba",
    "apply",
    "convert",
    "diff",
    "repair",
    "template",
    "capabilities",
    "help",
    "doctor",
    "find",
    "robot-docs",
    "agent",
    "agent-triage",
    "completion",
    "conformance",
    "serve",
    "mcp",
    "version",
];

pub(crate) const CAPABILITY_FILTER_ALIASES: &[(&str, &str)] = &[
    ("slides", "slide"),
    ("shapes", "shape"),
    ("animations", "animation"),
    ("masters", "master"),
    ("layouts", "layout"),
    ("placeholders", "placeholder"),
    ("sheets", "sheet"),
    ("ranges", "range"),
    ("conditional-formats", "conditional-format"),
    ("conditional-formatting", "conditional-format"),
    ("cf", "conditional-format"),
    ("data-validations", "data-validation"),
    ("dv", "data-validation"),
    ("cells", "cell"),
    ("hyperlinks", "hyperlink"),
    ("tables", "table"),
    ("pivots", "pivot"),
    ("names", "name"),
    ("blocks", "block"),
    ("paragraphs", "paragraph"),
    ("styles", "style"),
    ("themes", "theme"),
    ("comments", "comment"),
    ("charts", "chart"),
    ("fields", "field"),
    ("headers", "header"),
    ("footers", "footer"),
    ("images", "image"),
    ("modules", "module"),
    ("macros", "module"),
    ("macro", "module"),
];

pub(crate) const CONDITIONAL_FORMAT_TOPIC_ALIASES: &[&str] =
    &["conditional-format", "conditional-formatting", "cf"];
pub(crate) const DATA_VALIDATION_TOPIC_ALIASES: &[&str] = &["data-validation", "dv"];

pub(crate) fn normalize_capability_filter(raw: &str) -> String {
    let mut filter = raw.trim().to_ascii_lowercase().replace('_', "-");
    if let Some(stripped) = filter.strip_prefix("ooxml ") {
        filter = stripped.to_string();
    }
    CAPABILITY_FILTER_ALIASES
        .iter()
        .find_map(|(alias, canonical)| (*alias == filter).then_some((*canonical).to_string()))
        .unwrap_or(filter)
}

pub(crate) fn capability_filter_aliases_json() -> Value {
    json!(
        CAPABILITY_FILTER_ALIASES
            .iter()
            .map(|(alias, canonical)| json!({
                "alias": alias,
                "canonical": canonical
            }))
            .collect::<Vec<_>>()
    )
}

pub(crate) fn capability_filter_alias_strings() -> Vec<String> {
    CAPABILITY_FILTER_ALIASES
        .iter()
        .map(|(alias, canonical)| format!("{alias} -> {canonical}"))
        .collect()
}

pub(crate) fn capability_filter_suggestions(filter: &str) -> Vec<String> {
    let mut suggestions = Vec::new();
    for candidate in capability_known_filters() {
        if candidate.contains(filter) || filter.contains(&candidate) {
            suggestions.push(candidate);
        }
    }
    if suggestions.is_empty() {
        suggestions.extend([
            "pptx".to_string(),
            "xlsx".to_string(),
            "docx".to_string(),
            "slide".to_string(),
            "sheet".to_string(),
            "range".to_string(),
            "conditional-format".to_string(),
        ]);
    }
    suggestions.sort();
    suggestions.dedup();
    suggestions.truncate(8);
    suggestions
}

pub(crate) fn capability_known_filters() -> Vec<String> {
    let mut filters = Vec::new();
    filters.extend(
        CAPABILITY_COMMAND_FAMILY_FILTERS
            .iter()
            .map(|filter| (*filter).to_string()),
    );
    filters.extend(
        CAPABILITY_OBJECT_KINDS
            .iter()
            .map(|filter| (*filter).to_string()),
    );
    filters.extend(
        CAPABILITY_FILTER_ALIASES
            .iter()
            .map(|(alias, _)| (*alias).to_string()),
    );
    filters.sort();
    filters.dedup();
    filters
}

pub(crate) fn is_command_family_filter(filter: &str) -> bool {
    CAPABILITY_COMMAND_FAMILY_FILTERS.contains(&filter)
}
