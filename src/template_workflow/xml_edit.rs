use crate::{CliError, CliResult, local_name, xml_attr_escape, xml_token_name};

pub(super) fn first_element_span(
    xml: &str,
    wanted: &str,
    range_start: usize,
    range_end: usize,
) -> Option<(usize, usize)> {
    let mut cursor = range_start;
    while cursor < range_end {
        let relative_start = xml[cursor..range_end].find('<')?;
        let tag_start = cursor + relative_start;
        let relative_end = xml[tag_start..range_end].find('>')?;
        let tag_end = tag_start + relative_end;
        let token = xml[tag_start + 1..tag_end].trim_start();
        if token.starts_with('/') || token.starts_with('?') || token.starts_with('!') {
            cursor = tag_end + 1;
            continue;
        }
        let name = xml_token_name(token)?;
        let self_closing = token.trim_end().ends_with('/');
        if local_name(name.as_bytes()) == wanted {
            if self_closing {
                return Some((tag_start, tag_end + 1));
            }
            return find_matching_element_end(xml, wanted, tag_end + 1, range_end)
                .map(|end| (tag_start, end));
        }
        cursor = tag_end + 1;
    }
    None
}

fn find_matching_element_end(
    xml: &str,
    wanted: &str,
    range_start: usize,
    range_end: usize,
) -> Option<usize> {
    let mut depth = 1usize;
    let mut cursor = range_start;
    while cursor < range_end {
        let relative_start = xml[cursor..range_end].find('<')?;
        let tag_start = cursor + relative_start;
        let relative_end = xml[tag_start..range_end].find('>')?;
        let tag_end = tag_start + relative_end;
        let token = xml[tag_start + 1..tag_end].trim_start();
        if token.starts_with('?') || token.starts_with('!') {
            cursor = tag_end + 1;
            continue;
        }
        if let Some(name) = xml_token_name(token)
            && local_name(name.as_bytes()) == wanted
        {
            if token.starts_with('/') {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(tag_end + 1);
                }
            } else if !token.trim_end().ends_with('/') {
                depth += 1;
            }
        }
        cursor = tag_end + 1;
    }
    None
}

pub(super) fn set_attr_on_element(
    xml: &str,
    element_start: usize,
    attr_name: &str,
    value: &str,
) -> CliResult<String> {
    let tag_end = xml[element_start..]
        .find('>')
        .map(|offset| element_start + offset + 1)
        .ok_or_else(|| CliError::unexpected("invalid XML start tag"))?;
    let start_tag = &xml[element_start..tag_end];
    let replacement = set_attr_on_start_tag(start_tag, attr_name, value)?;
    Ok(replace_span(xml, element_start, tag_end, &replacement))
}

fn set_attr_on_start_tag(start_tag: &str, attr_name: &str, value: &str) -> CliResult<String> {
    let Some(open_end) = start_tag.find('>') else {
        return Err(CliError::unexpected("invalid XML start tag"));
    };
    let Some(token_name) = xml_token_name(&start_tag[1..open_end]) else {
        return Err(CliError::unexpected("invalid XML start tag"));
    };
    let mut cursor = 1 + token_name.len();
    while cursor < open_end {
        while cursor < open_end && start_tag.as_bytes()[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor >= open_end || start_tag.as_bytes()[cursor] == b'/' {
            break;
        }
        let name_start = cursor;
        while cursor < open_end {
            let byte = start_tag.as_bytes()[cursor];
            if byte == b'=' || byte.is_ascii_whitespace() || byte == b'/' {
                break;
            }
            cursor += 1;
        }
        let name_end = cursor;
        while cursor < open_end && start_tag.as_bytes()[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor >= open_end || start_tag.as_bytes()[cursor] != b'=' {
            continue;
        }
        cursor += 1;
        while cursor < open_end && start_tag.as_bytes()[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor >= open_end {
            break;
        }
        let quote = start_tag.as_bytes()[cursor];
        if quote != b'"' && quote != b'\'' {
            continue;
        }
        cursor += 1;
        let value_start = cursor;
        while cursor < open_end && start_tag.as_bytes()[cursor] != quote {
            cursor += 1;
        }
        if cursor >= open_end {
            break;
        }
        let value_end = cursor;
        cursor += 1;
        let existing_name = &start_tag[name_start..name_end];
        if local_name(existing_name.as_bytes()) == attr_name {
            if &start_tag[value_start..value_end] == value {
                return Ok(start_tag.to_string());
            }
            let mut out = String::with_capacity(start_tag.len() + value.len());
            out.push_str(&start_tag[..value_start]);
            out.push_str(&xml_attr_escape(value));
            out.push_str(&start_tag[value_end..]);
            return Ok(out);
        }
    }

    let insert_at = if start_tag[..open_end].trim_end().ends_with('/') {
        start_tag[..open_end]
            .rfind('/')
            .ok_or_else(|| CliError::unexpected("invalid XML start tag"))?
    } else {
        open_end
    };
    let mut out = String::with_capacity(start_tag.len() + attr_name.len() + value.len() + 4);
    out.push_str(&start_tag[..insert_at]);
    out.push(' ');
    out.push_str(attr_name);
    out.push_str("=\"");
    out.push_str(&xml_attr_escape(value));
    out.push('"');
    out.push_str(&start_tag[insert_at..]);
    Ok(out)
}

pub(super) fn xml_open_tag(fragment: &str, open_end: usize) -> String {
    let start_tag = &fragment[..=open_end];
    if !start_tag.trim_end().ends_with("/>") {
        return start_tag.to_string();
    }
    let slash = start_tag
        .rfind('/')
        .unwrap_or_else(|| start_tag.len().saturating_sub(1));
    let mut out = String::new();
    out.push_str(&start_tag[..slash]);
    out.push('>');
    out
}

pub(super) fn replace_span(xml: &str, start: usize, end: usize, replacement: &str) -> String {
    let mut out = String::with_capacity(xml.len() - (end - start) + replacement.len());
    out.push_str(&xml[..start]);
    out.push_str(replacement);
    out.push_str(&xml[end..]);
    out
}

pub(super) fn insert_at_index(xml: &str, index: usize, insertion: &str) -> String {
    let mut out = String::with_capacity(xml.len() + insertion.len());
    out.push_str(&xml[..index]);
    out.push_str(insertion);
    out.push_str(&xml[index..]);
    out
}

pub(super) fn tag_prefix(tag_name: &str) -> String {
    tag_name
        .split_once(':')
        .map(|(prefix, _)| prefix.to_string())
        .unwrap_or_default()
}

pub(super) fn root_prefix(xml: &str, root_open_end: usize) -> String {
    xml_token_name(xml[1..root_open_end].trim_start())
        .map(tag_prefix)
        .unwrap_or_default()
}

pub(super) fn namespace_prefix(xml: &str, uri: &str) -> Option<String> {
    let root_open_end = xml.find('>')?;
    let root_tag = &xml[..root_open_end];
    for quote in ['"', '\''] {
        let needle = format!("{quote}{uri}{quote}");
        let Some(value_start) = root_tag.find(&needle) else {
            continue;
        };
        let before = &root_tag[..value_start];
        let eq = before.rfind('=')?;
        let attr_name = before[..eq].split_whitespace().last()?;
        if attr_name == "xmlns" {
            return Some(String::new());
        }
        if let Some(prefix) = attr_name.strip_prefix("xmlns:") {
            return Some(prefix.to_string());
        }
    }
    None
}

pub(super) fn qualified_name(prefix: &str, local: &str) -> String {
    if prefix.is_empty() {
        local.to_string()
    } else {
        format!("{prefix}:{local}")
    }
}
