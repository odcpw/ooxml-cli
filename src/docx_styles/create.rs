use std::collections::{BTreeMap, BTreeSet};

use super::{DocxStyleInfo, DocxStyleTarget};
use crate::docx_authoring::styles::{
    BUILT_IN_STYLES, BuiltInNumbering, BuiltInStyle, built_in_style_fragment, styles_xml,
};
use crate::{
    CliError, CliResult, add_relationship_to_xml, allocate_relationship_id, content_type_for_part,
    ensure_content_type_override, is_docx_numbering_part, relationship_entries,
    relationship_entries_from_xml, relationship_target_from_source_to_target,
    relationships_part_for, resolve_relationship_target, zip_entry_names, zip_text,
};

const STYLES_REL: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles";
const NUMBERING_REL: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering";
const STYLES_CONTENT_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml";
const NUMBERING_CONTENT_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml";
const DEFAULT_STYLES_PART: &str = "word/styles.xml";
const DEFAULT_NUMBERING_PART: &str = "word/numbering.xml";

pub(crate) struct PreparedDocxStyle {
    pub(crate) style_id: String,
    pub(crate) created: bool,
    pub(crate) overrides: BTreeMap<String, String>,
}

pub(super) fn prepare_docx_style(
    file: &str,
    document_part: &str,
    styles_part: Option<&str>,
    styles: &[DocxStyleInfo],
    requested: &str,
    target: DocxStyleTarget,
    create_style: bool,
) -> CliResult<PreparedDocxStyle> {
    let requested = requested.trim();
    if requested.is_empty() {
        if create_style {
            return Err(CliError::invalid_args(
                "--create-style requires a non-empty --style",
            ));
        }
        return Ok(PreparedDocxStyle {
            style_id: String::new(),
            created: false,
            overrides: BTreeMap::new(),
        });
    }

    if let Some(style) = resolve_existing_style(styles, requested)? {
        require_style_type(style, target)?;
        return Ok(PreparedDocxStyle {
            style_id: style.style_id.clone(),
            created: false,
            overrides: BTreeMap::new(),
        });
    }

    if !create_style {
        return Err(style_not_found(styles, requested, target));
    }
    let built_in = resolve_built_in_style(requested)
        .ok_or_else(|| style_not_found_with_create_hint(styles, requested, target))?;
    if built_in.style_type != target.required_style_type() {
        return Err(CliError::invalid_args(format!(
            "style type mismatch: {:?} is a {} style but {} target requires a {} style",
            built_in.id,
            built_in.style_type,
            target.as_str(),
            target.required_style_type(),
        )));
    }

    let entries = zip_entry_names(file)?;
    let mut overrides = BTreeMap::new();
    if let Some(styles_part) = styles_part {
        let styles_part = styles_part.trim_start_matches('/');
        let mut xml = zip_text(file, styles_part)?;
        let base_style = match built_in.style_type {
            "paragraph" if built_in.id != "Normal" => Some("Normal"),
            "character" => Some("DefaultParagraphFont"),
            "table" => Some("TableNormal"),
            _ => None,
        };
        if let Some(base_style) = base_style
            && !styles.iter().any(|style| style.style_id == base_style)
        {
            xml = insert_before_root_close(
                &xml,
                "styles",
                built_in_style_fragment(base_style).expect("built-in base style"),
            )?;
        }
        let mut fragment = built_in_style_fragment(built_in.id)
            .expect("built-in style metadata and XML stay aligned")
            .to_string();
        if let Some(numbering_kind) = built_in.numbering {
            let num_id = ensure_numbering(
                file,
                document_part,
                &entries,
                numbering_kind,
                &mut overrides,
            )?;
            let scaffold_num_id = match numbering_kind {
                BuiltInNumbering::Bullet => 1,
                BuiltInNumbering::Number => 2,
            };
            fragment = fragment.replace(
                &format!(r#"<w:numId w:val="{scaffold_num_id}"/>"#),
                &format!(r#"<w:numId w:val="{num_id}"/>"#),
            );
        }
        xml = insert_before_root_close(&xml, "styles", &fragment)?;
        overrides.insert(styles_part.to_string(), xml);
    } else {
        overrides.insert(DEFAULT_STYLES_PART.to_string(), styles_xml().to_string());
        ensure_part_registration(
            file,
            document_part,
            DEFAULT_STYLES_PART,
            STYLES_REL,
            STYLES_CONTENT_TYPE,
            &entries,
            &mut overrides,
        )?;
        ensure_numbering(
            file,
            document_part,
            &entries,
            BuiltInNumbering::Bullet,
            &mut overrides,
        )?;
    }

    Ok(PreparedDocxStyle {
        style_id: built_in.id.to_string(),
        created: true,
        overrides,
    })
}

fn resolve_existing_style<'a>(
    styles: &'a [DocxStyleInfo],
    requested: &str,
) -> CliResult<Option<&'a DocxStyleInfo>> {
    if let Some(style) = styles.iter().find(|style| style.style_id == requested) {
        return Ok(Some(style));
    }
    let wanted = normalized_style_name(requested);
    let matches: Vec<&DocxStyleInfo> = styles
        .iter()
        .filter(|style| !style.name.is_empty() && normalized_style_name(&style.name) == wanted)
        .collect();
    match matches.as_slice() {
        [] => Ok(None),
        [style] => Ok(Some(*style)),
        _ => Err(CliError::invalid_args(format!(
            "style name {requested:?} is ambiguous; matching canonical ids: [{}]",
            matches
                .iter()
                .map(|style| style.style_id.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        ))),
    }
}

fn resolve_built_in_style(requested: &str) -> Option<&'static BuiltInStyle> {
    let wanted = normalized_style_name(requested);
    BUILT_IN_STYLES.iter().find(|style| {
        style.id == requested
            || normalized_style_name(style.id) == wanted
            || normalized_style_name(style.name) == wanted
    })
}

fn require_style_type(style: &DocxStyleInfo, target: DocxStyleTarget) -> CliResult<()> {
    let wanted = target.required_style_type();
    if style.style_type == wanted {
        return Ok(());
    }
    Err(CliError::invalid_args(format!(
        "style type mismatch: {:?} is a {} style but {} target requires a {} style",
        style.style_id,
        style.style_type,
        target.as_str(),
        wanted,
    )))
}

fn style_not_found(styles: &[DocxStyleInfo], requested: &str, target: DocxStyleTarget) -> CliError {
    let wanted = target.required_style_type();
    let mut available: Vec<&DocxStyleInfo> = styles
        .iter()
        .filter(|style| style.style_type == wanted)
        .collect();
    available.sort_by(|left, right| left.style_id.cmp(&right.style_id));
    let list = available
        .iter()
        .map(|style| {
            if style.name.is_empty() || style.name == style.style_id {
                style.style_id.clone()
            } else {
                format!("{} ({})", style.style_id, style.name)
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    let nearest = nearest_style(&available, requested)
        .map(|style| format!("; nearest match: {} ({})", style.style_id, style.name))
        .unwrap_or_default();
    let available = if list.is_empty() {
        format!("no {wanted} styles are defined")
    } else {
        format!("available {wanted} styles: [{list}]")
    };
    CliError::target_not_found(format!(
        "style not found: {requested:?} ({wanted}); {available}{nearest}"
    ))
}

fn style_not_found_with_create_hint(
    styles: &[DocxStyleInfo],
    requested: &str,
    target: DocxStyleTarget,
) -> CliError {
    let mut error = style_not_found(styles, requested, target);
    error.message.push_str(&format!(
        "; --create-style supports built-ins: [{}]",
        BUILT_IN_STYLES
            .iter()
            .filter(|style| style.style_type == target.required_style_type())
            .map(|style| style.id)
            .collect::<Vec<_>>()
            .join(" ")
    ));
    error
}

fn nearest_style<'a>(styles: &'a [&DocxStyleInfo], requested: &str) -> Option<&'a DocxStyleInfo> {
    let wanted = normalized_style_name(requested);
    styles
        .iter()
        .map(|style| {
            let id_distance = levenshtein(&wanted, &normalized_style_name(&style.style_id));
            let name_distance = levenshtein(&wanted, &normalized_style_name(&style.name));
            (*style, id_distance.min(name_distance))
        })
        .min_by(|(left, left_distance), (right, right_distance)| {
            left_distance
                .cmp(right_distance)
                .then_with(|| left.style_id.cmp(&right.style_id))
        })
        .map(|(style, _)| style)
}

fn normalized_style_name(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect()
}

fn levenshtein(left: &str, right: &str) -> usize {
    let right_chars: Vec<char> = right.chars().collect();
    let mut previous: Vec<usize> = (0..=right_chars.len()).collect();
    let mut current = vec![0; right_chars.len() + 1];
    for (left_index, left_character) in left.chars().enumerate() {
        current[0] = left_index + 1;
        for (right_index, right_character) in right_chars.iter().enumerate() {
            let substitution =
                previous[right_index] + usize::from(left_character != *right_character);
            current[right_index + 1] = (current[right_index] + 1)
                .min(previous[right_index + 1] + 1)
                .min(substitution);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[right_chars.len()]
}

fn ensure_numbering(
    file: &str,
    document_part: &str,
    entries: &[String],
    kind: BuiltInNumbering,
    overrides: &mut BTreeMap<String, String>,
) -> CliResult<u32> {
    let numbering_part = find_numbering_part(file, document_part, entries)?;
    let Some(numbering_part) = numbering_part else {
        overrides.insert(
            DEFAULT_NUMBERING_PART.to_string(),
            crate::docx_authoring::numbering::numbering_xml().to_string(),
        );
        ensure_part_registration(
            file,
            document_part,
            DEFAULT_NUMBERING_PART,
            NUMBERING_REL,
            NUMBERING_CONTENT_TYPE,
            entries,
            overrides,
        )?;
        return Ok(match kind {
            BuiltInNumbering::Bullet => 1,
            BuiltInNumbering::Number => 2,
        });
    };
    let numbering_part = numbering_part.trim_start_matches('/').to_string();
    let xml = overrides
        .get(&numbering_part)
        .cloned()
        .unwrap_or(zip_text(file, &numbering_part)?);
    let (abstract_ids, num_ids) = numbering_ids(&xml)?;
    let next_abstract = abstract_ids.iter().next_back().copied().unwrap_or(0) + 1;
    let next_num = num_ids.iter().next_back().copied().unwrap_or(0) + 1;
    let source_abstract = match kind {
        BuiltInNumbering::Bullet => 0,
        BuiltInNumbering::Number => 1,
    };
    let scaffold = crate::docx_authoring::numbering::numbering_xml();
    let abstract_fragment = xml_element_fragment(
        scaffold,
        &format!(r#"<w:abstractNum w:abstractNumId="{source_abstract}">"#),
        "w:abstractNum",
    )?
    .replace(
        &format!(r#"w:abstractNumId="{source_abstract}""#),
        &format!(r#"w:abstractNumId="{next_abstract}""#),
    );
    let num_fragment = format!(
        r#"<w:num w:numId="{next_num}"><w:abstractNumId w:val="{next_abstract}"/></w:num>"#
    );
    let additions = format!("{abstract_fragment}{num_fragment}");
    overrides.insert(
        numbering_part,
        insert_before_root_close(&xml, "numbering", &additions)?,
    );
    Ok(next_num)
}

fn find_numbering_part(
    file: &str,
    document_part: &str,
    entries: &[String],
) -> CliResult<Option<String>> {
    let rels_part = relationships_part_for(document_part);
    let document_uri = format!("/{}", document_part.trim_start_matches('/'));
    for relationship in relationship_entries(file, &rels_part).unwrap_or_default() {
        if relationship.target_mode != "External"
            && (relationship.rel_type == NUMBERING_REL
                || relationship.rel_type.ends_with("/numbering"))
        {
            return Ok(Some(resolve_relationship_target(
                &document_uri,
                &relationship.target,
            )));
        }
    }
    for entry in entries {
        let uri = format!("/{}", entry.trim_start_matches('/'));
        let content_type = content_type_for_part(file, &uri).unwrap_or_default();
        if is_docx_numbering_part(&uri, &content_type) {
            return Ok(Some(uri));
        }
    }
    Ok(None)
}

fn ensure_part_registration(
    file: &str,
    document_part: &str,
    target_part: &str,
    relationship_type: &str,
    content_type: &str,
    entries: &[String],
    overrides: &mut BTreeMap<String, String>,
) -> CliResult<()> {
    let rels_part = relationships_part_for(document_part);
    let rels = overrides.get(&rels_part).cloned().unwrap_or_else(|| {
        if entries.iter().any(|entry| entry == &rels_part) {
            zip_text(file, &rels_part).unwrap_or_default()
        } else {
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"></Relationships>"#.to_string()
        }
    });
    let relationships = relationship_entries_from_xml(&rels);
    if !relationships
        .iter()
        .any(|relationship| relationship.rel_type == relationship_type)
    {
        let id = allocate_relationship_id(&relationships);
        let target = relationship_target_from_source_to_target(document_part, target_part);
        overrides.insert(
            rels_part,
            add_relationship_to_xml(rels, &id, relationship_type, &target),
        );
    }
    let content_types = overrides
        .get("[Content_Types].xml")
        .cloned()
        .unwrap_or(zip_text(file, "[Content_Types].xml")?);
    overrides.insert(
        "[Content_Types].xml".to_string(),
        ensure_content_type_override(content_types, target_part, content_type)?,
    );
    Ok(())
}

fn insert_before_root_close(xml: &str, root: &str, fragment: &str) -> CliResult<String> {
    let close = format!("</w:{root}>");
    let position = xml
        .rfind(&close)
        .ok_or_else(|| CliError::unexpected(format!("invalid DOCX {root} part")))?;
    let mut updated = String::with_capacity(xml.len() + fragment.len());
    updated.push_str(&xml[..position]);
    updated.push_str(fragment);
    updated.push_str(&xml[position..]);
    Ok(updated)
}

fn numbering_ids(xml: &str) -> CliResult<(BTreeSet<u32>, BTreeSet<u32>)> {
    let mut reader = quick_xml::Reader::from_str(xml);
    let mut abstract_ids = BTreeSet::new();
    let mut num_ids = BTreeSet::new();
    loop {
        match reader.read_event() {
            Ok(quick_xml::events::Event::Start(element))
            | Ok(quick_xml::events::Event::Empty(element)) => {
                match crate::local_name(element.name().as_ref()) {
                    "abstractNum" => collect_u32_attr(&element, "abstractNumId", &mut abstract_ids),
                    "num" => collect_u32_attr(&element, "numId", &mut num_ids),
                    _ => {}
                }
            }
            Ok(quick_xml::events::Event::Eof) => break,
            Err(error) => {
                return Err(CliError::unexpected(format!(
                    "failed to parse DOCX numbering part: {error}"
                )));
            }
            _ => {}
        }
    }
    Ok((abstract_ids, num_ids))
}

fn collect_u32_attr(
    element: &quick_xml::events::BytesStart<'_>,
    name: &str,
    values: &mut BTreeSet<u32>,
) {
    if let Some(value) = crate::attr(element, name).and_then(|value| value.parse().ok()) {
        values.insert(value);
    }
}

fn xml_element_fragment<'a>(xml: &'a str, opening: &str, tag: &str) -> CliResult<&'a str> {
    let start = xml
        .find(opening)
        .ok_or_else(|| CliError::unexpected(format!("built-in {tag} fragment not found")))?;
    let close = format!("</{tag}>");
    let relative_end = xml[start..]
        .find(&close)
        .ok_or_else(|| CliError::unexpected(format!("built-in {tag} fragment is malformed")))?;
    Ok(&xml[start..start + relative_end + close.len()])
}

#[cfg(test)]
mod tests {
    use super::{levenshtein, normalized_style_name};

    #[test]
    fn style_name_matching_ignores_case_and_space() {
        assert_eq!(normalized_style_name(" Heading 1 "), "heading1");
        assert_eq!(normalized_style_name("HEADING1"), "heading1");
    }

    #[test]
    fn nearest_style_distance_is_stable() {
        assert_eq!(levenshtein("heading5", "heading4"), 1);
        assert_eq!(levenshtein("quote", "subtitle"), 5);
    }
}
