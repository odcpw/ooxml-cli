use quick_xml::Reader;
use quick_xml::events::Event;

use crate::{CliError, CliResult, attr, docx_body_content_bounds, docx_body_tag, local_name};
struct OpenXmlDeleteElement {
    name: String,
    start: usize,
    delete_self: bool,
    contains_target_comment_reference: bool,
}

pub(super) fn remove_docx_comment_markers_xml(
    document_xml: &str,
    target_id: i64,
) -> CliResult<(String, bool)> {
    let body_tag = docx_body_tag(document_xml)?;
    let (content_start, content_end) = docx_body_content_bounds(document_xml, &body_tag)?;
    let body_xml = &document_xml[content_start..content_end];
    let target = target_id.to_string();
    let mut reader = Reader::from_str(body_xml);
    reader.config_mut().trim_text(false);
    let mut stack = Vec::<OpenXmlDeleteElement>::new();
    let mut ranges = Vec::<(usize, usize)>::new();

    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local_name(e.name().as_ref()).to_string();
                let is_target_marker =
                    matches!(name.as_str(), "commentRangeStart" | "commentRangeEnd")
                        && attr(&e, "id").is_some_and(|id| id == target);
                let is_target_reference =
                    name == "commentReference" && attr(&e, "id").is_some_and(|id| id == target);
                let reference_has_run_parent =
                    is_target_reference && mark_nearest_open_word_run(&mut stack);
                stack.push(OpenXmlDeleteElement {
                    name,
                    start: content_start + before,
                    delete_self: is_target_marker
                        || (is_target_reference && !reference_has_run_parent),
                    contains_target_comment_reference: false,
                });
            }
            Ok(Event::Empty(e)) => {
                let after = reader.buffer_position() as usize;
                let name = local_name(e.name().as_ref()).to_string();
                let is_target_marker =
                    matches!(name.as_str(), "commentRangeStart" | "commentRangeEnd")
                        && attr(&e, "id").is_some_and(|id| id == target);
                if is_target_marker {
                    ranges.push((content_start + before, content_start + after));
                    continue;
                }
                let is_target_reference =
                    name == "commentReference" && attr(&e, "id").is_some_and(|id| id == target);
                if is_target_reference && !mark_nearest_open_word_run(&mut stack) {
                    ranges.push((content_start + before, content_start + after));
                }
            }
            Ok(Event::End(_)) => {
                let after = reader.buffer_position() as usize;
                let Some(element) = stack.pop() else {
                    continue;
                };
                if element.delete_self || element.contains_target_comment_reference {
                    ranges.push((element.start, content_start + after));
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }

    if ranges.is_empty() {
        return Ok((document_xml.to_string(), false));
    }
    Ok((delete_xml_ranges(document_xml, ranges)?, true))
}

fn mark_nearest_open_word_run(stack: &mut [OpenXmlDeleteElement]) -> bool {
    if let Some(run) = stack.iter_mut().rev().find(|element| element.name == "r") {
        run.contains_target_comment_reference = true;
        true
    } else {
        false
    }
}

fn delete_xml_ranges(xml: &str, mut ranges: Vec<(usize, usize)>) -> CliResult<String> {
    ranges.retain(|(start, end)| start < end && *end <= xml.len());
    if ranges.is_empty() {
        return Ok(xml.to_string());
    }
    ranges.sort_by_key(|(start, end)| (*start, std::cmp::Reverse(*end)));
    let mut merged = Vec::<(usize, usize)>::new();
    for (start, end) in ranges {
        if let Some((_, current_end)) = merged.last_mut()
            && start <= *current_end
        {
            *current_end = (*current_end).max(end);
            continue;
        }
        merged.push((start, end));
    }
    let mut out = xml.to_string();
    for (start, end) in merged.into_iter().rev() {
        out.replace_range(start..end, "");
    }
    Ok(out)
}
