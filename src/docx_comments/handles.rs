use super::docx_comment_element_spans_by_id;
use crate::{
    CliError, CliResult, EXIT_INVALID_ARGS, EXIT_TARGET_NOT_FOUND, HANDLE_AMBIGUOUS,
    HANDLE_FORMAT_MISMATCH, HANDLE_MALFORMED, HANDLE_STALE, docx_handle_error,
};

pub(super) fn resolve_docx_comment_handle_id(comments_xml: &str, handle: &str) -> CliResult<i64> {
    let comment_id = parse_docx_comment_handle_id(handle)?;
    let spans = docx_comment_element_spans_by_id(comments_xml, comment_id)?;
    match spans.len() {
        0 => Err(docx_handle_error(
            EXIT_TARGET_NOT_FOUND,
            HANDLE_STALE,
            format!("no comment with w:id {comment_id} in document"),
            handle,
        )),
        1 => Ok(comment_id),
        count => Err(docx_handle_error(
            EXIT_TARGET_NOT_FOUND,
            HANDLE_AMBIGUOUS,
            format!(
                "w:id {comment_id} is not unique ({count} comments share it); cannot resolve to a single comment"
            ),
            handle,
        )),
    }
}

fn parse_docx_comment_handle_id(handle: &str) -> CliResult<i64> {
    let trimmed = handle.trim();
    let Some(body) = trimmed.strip_prefix("H:") else {
        return Err(docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_MALFORMED,
            "missing handle version prefix \"H:\"",
            handle,
        ));
    };
    let segments: Vec<&str> = body.split('/').collect();
    if segments.len() != 3 {
        return Err(docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_MALFORMED,
            "handle must be H:docx/<scope>/<class>:<objref>",
            handle,
        ));
    }
    if segments[0].is_empty() {
        return Err(docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_MALFORMED,
            "empty format tag",
            handle,
        ));
    }
    if segments[0] != "docx" {
        return Err(docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_FORMAT_MISMATCH,
            format!(
                "handle format tag {:?} does not match package format {:?}",
                segments[0], "docx"
            ),
            handle,
        ));
    }
    let Some((class, objref)) = segments[2].split_once(':') else {
        return Err(docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_MALFORMED,
            format!("object segment {:?} must be <class>:<objref>", segments[2]),
            handle,
        ));
    };
    if segments[1] != "pt:doc" {
        return Err(docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_MALFORMED,
            format!(
                "comment handle scope must be {:?}, got {:?}",
                "pt:doc", segments[1]
            ),
            handle,
        ));
    }
    if class != "comment" {
        return Err(CliError::invalid_args(
            "--handle must be a comment handle (H:docx/pt:doc/comment:n:<id>)",
        ));
    }
    let Some((tag, value)) = objref.split_once(':') else {
        return Err(docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_MALFORMED,
            format!("comment objref: objref {objref:?} must be n:<id>"),
            handle,
        ));
    };
    if tag != "n" {
        return Err(docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_MALFORMED,
            format!("comment objref: unsupported objref tag {tag:?} (expected native id \"n\")"),
            handle,
        ));
    }
    if value.is_empty() {
        return Err(docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_MALFORMED,
            "comment objref: empty native id",
            handle,
        ));
    }
    let id = value.parse::<i64>().map_err(|err| {
        docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_MALFORMED,
            format!("comment objref: invalid native id {value:?}: {err}"),
            handle,
        )
    })?;
    if id < 0 {
        return Err(docx_handle_error(
            EXIT_INVALID_ARGS,
            HANDLE_MALFORMED,
            format!("comment objref: native id must be non-negative, got {id}"),
            handle,
        ));
    }
    Ok(id)
}
