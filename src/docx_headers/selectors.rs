use serde_json::{Value, json};

use crate::{CliError, CliResult};

#[derive(Clone, Debug, Default)]
pub(super) struct DocxHeaderFooterRefInfo {
    pub(super) kind: String,
    pub(super) id: String,
    pub(super) ref_type: String,
    pub(super) section: i64,
    pub(super) primary_selector: String,
    pub(super) selectors: Vec<String>,
    pub(super) part_uri: String,
}

#[derive(Default)]
pub(super) struct DocxHeaderFooterSelector {
    pub(super) kind: String,
    pub(super) id: String,
    pub(super) ref_type: String,
    pub(super) section: i64,
    pub(super) part_uri: String,
    pub(super) paragraph_index: i64,
}

pub(super) fn docx_header_footer_ref_json(
    kind: &str,
    id: &str,
    ref_type: &str,
    section: usize,
    part_uri: &str,
    content_type: &str,
) -> Value {
    let primary_selector = format!("{kind}:{section}:{ref_type}");
    let mut selectors = vec![primary_selector.clone()];
    if !id.is_empty() {
        selectors.push(format!("id:{id}"));
        selectors.push(id.to_string());
    }
    if !part_uri.is_empty() {
        selectors.push(format!("part:{part_uri}"));
        selectors.push(part_uri.to_string());
    }
    json!({
        "kind": kind,
        "id": id,
        "type": ref_type,
        "section": section,
        "primarySelector": primary_selector,
        "selectors": selectors,
        "partUri": part_uri,
        "contentType": content_type,
    })
}

pub(crate) fn normalize_docx_header_footer_show_type(value: &str) -> CliResult<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "" | "default" => Ok("default".to_string()),
        "first" => Ok("first".to_string()),
        "even" => Ok("even".to_string()),
        _ => Err(CliError::invalid_args(
            "--type must be one of default, first, even",
        )),
    }
}

pub(super) fn parse_docx_header_footer_selector(
    command_kind: &str,
    raw: &str,
) -> CliResult<DocxHeaderFooterSelector> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(CliError::invalid_args("--selector cannot be empty"));
    }
    let (base, paragraph_index) = split_docx_header_footer_paragraph_selector(raw)?;
    let mut selector = DocxHeaderFooterSelector {
        kind: command_kind.to_string(),
        ref_type: "default".to_string(),
        paragraph_index,
        ..DocxHeaderFooterSelector::default()
    };
    if let Some(id) = base.strip_prefix("id:") {
        if id.is_empty() {
            return Err(CliError::invalid_args(
                "--selector id:<relId> cannot be empty",
            ));
        }
        selector.id = id.to_string();
        return Ok(selector);
    }
    if let Some(part_uri) = base.strip_prefix("part:") {
        if part_uri.is_empty() {
            return Err(CliError::invalid_args(
                "--selector part:<partUri> cannot be empty",
            ));
        }
        selector.part_uri = part_uri.to_string();
        return Ok(selector);
    }
    if base.starts_with('/') {
        selector.part_uri = base.to_string();
        return Ok(selector);
    }
    if base.starts_with("rId") {
        selector.id = base.to_string();
        return Ok(selector);
    }
    if let Some(rest) = base.strip_prefix("section:") {
        let parts = rest.split(':').collect::<Vec<_>>();
        if parts.len() != 3 || parts[1] != "type" {
            return Err(CliError::invalid_args(
                "--selector section form must be section:<n>:type:<default|first|even>",
            ));
        }
        selector.section = parse_positive_i64(parts[0], "selector section")?;
        selector.ref_type = normalize_docx_header_footer_show_type(parts[2])?;
        return Ok(selector);
    }

    let parts = base.split(':').collect::<Vec<_>>();
    if parts.len() == 3 && (parts[0] == "header" || parts[0] == "footer") {
        if parts[0] != command_kind {
            return Err(CliError::invalid_args(format!(
                "--selector kind {:?} does not match {command_kind} command",
                parts[0]
            )));
        }
        selector.kind = parts[0].to_string();
        selector.section = parse_positive_i64(parts[1], "selector section")?;
        selector.ref_type = normalize_docx_header_footer_show_type(parts[2])?;
        return Ok(selector);
    }

    Err(CliError::invalid_args(
        "--selector must be header:<section>:<type>, footer:<section>:<type>, section:<section>:type:<type>, id:<relId>, or part:<partUri>",
    ))
}

fn split_docx_header_footer_paragraph_selector(raw: &str) -> CliResult<(&str, i64)> {
    for marker in ["/paragraph:", "/p:"] {
        if let Some(index) = raw.rfind(marker) {
            let base = raw[..index].trim();
            let value = raw[index + marker.len()..].trim();
            if base.is_empty() {
                return Err(CliError::invalid_args(
                    "--selector paragraph suffix requires a header/footer selector before it",
                ));
            }
            let paragraph_index = parse_positive_i64(value, "selector paragraph")?;
            return Ok((base, paragraph_index));
        }
    }
    Ok((raw, 0))
}

fn parse_positive_i64(value: &str, label: &str) -> CliResult<i64> {
    let value = value.trim();
    if value.is_empty() {
        return Err(CliError::invalid_args(format!("{label} cannot be empty")));
    }
    let parsed = value
        .parse::<i64>()
        .map_err(|_| CliError::invalid_args(format!("{label} must be an integer")))?;
    if parsed < 1 {
        return Err(CliError::invalid_args(format!("{label} must be >= 1")));
    }
    Ok(parsed)
}

pub(super) fn resolve_docx_header_footer_selector(
    sections: &[Value],
    command_kind: &str,
    selector: &DocxHeaderFooterSelector,
) -> Option<DocxHeaderFooterRefInfo> {
    let kind = if selector.kind.is_empty() {
        command_kind
    } else {
        &selector.kind
    };
    let refs = docx_header_footer_refs(sections, kind);
    if !selector.id.is_empty() {
        return refs
            .into_iter()
            .find(|reference| reference.id == selector.id);
    }
    if !selector.part_uri.is_empty() {
        return refs
            .into_iter()
            .find(|reference| reference.part_uri == selector.part_uri);
    }
    let section = if selector.section > 0 {
        selector.section
    } else {
        sections
            .last()
            .and_then(|section| section["sectionIndex"].as_i64())
            .unwrap_or(0)
    };
    refs.into_iter()
        .find(|reference| reference.section == section && reference.ref_type == selector.ref_type)
}

fn docx_header_footer_refs(sections: &[Value], kind: &str) -> Vec<DocxHeaderFooterRefInfo> {
    let mut refs = Vec::new();
    for section in sections {
        let set = if kind == "footer" {
            &section["footers"]
        } else {
            &section["headers"]
        };
        for ref_type in ["default", "first", "even"] {
            if let Some(reference) = docx_header_footer_ref_info(&set[ref_type]) {
                refs.push(reference);
            }
        }
    }
    refs
}

fn docx_header_footer_ref_info(value: &Value) -> Option<DocxHeaderFooterRefInfo> {
    if value.is_null() {
        return None;
    }
    let selectors = value["selectors"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(ToString::to_string))
                .collect()
        })
        .unwrap_or_default();
    Some(DocxHeaderFooterRefInfo {
        kind: value["kind"].as_str()?.to_string(),
        id: value["id"].as_str().unwrap_or_default().to_string(),
        ref_type: value["type"].as_str().unwrap_or_default().to_string(),
        section: value["section"].as_i64().unwrap_or_default(),
        primary_selector: value["primarySelector"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        selectors,
        part_uri: value["partUri"].as_str().unwrap_or_default().to_string(),
    })
}

pub(super) fn docx_header_footer_ref_info_from_parts(
    kind: &str,
    id: &str,
    ref_type: &str,
    section: i64,
    part_uri: &str,
) -> DocxHeaderFooterRefInfo {
    let primary_selector = format!("{kind}:{section}:{ref_type}");
    let mut selectors = vec![primary_selector.clone()];
    if !id.is_empty() {
        selectors.push(format!("id:{id}"));
        selectors.push(id.to_string());
    }
    if !part_uri.is_empty() {
        selectors.push(format!("part:{part_uri}"));
        selectors.push(part_uri.to_string());
    }
    DocxHeaderFooterRefInfo {
        kind: kind.to_string(),
        id: id.to_string(),
        ref_type: ref_type.to_string(),
        section,
        primary_selector,
        selectors,
        part_uri: part_uri.to_string(),
    }
}

pub(super) fn docx_header_footer_paragraph_primary_selector(selector: &str, index: i64) -> String {
    if selector.is_empty() {
        String::new()
    } else {
        format!("{selector}/p:{index}")
    }
}

pub(super) fn docx_header_footer_paragraph_selectors(
    selectors: &[String],
    index: i64,
) -> Vec<String> {
    let mut out = Vec::with_capacity(selectors.len() * 2);
    for selector in selectors {
        out.push(format!("{selector}/p:{index}"));
        out.push(format!("{selector}/paragraph:{index}"));
    }
    out
}
