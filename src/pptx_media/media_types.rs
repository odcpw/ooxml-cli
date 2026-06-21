use std::path::Path;

use crate::{CliError, CliResult};

pub(super) fn resolve_media_kind(explicit: Option<&str>, ext: &str) -> CliResult<String> {
    if let Some(value) = explicit.map(str::trim).filter(|value| !value.is_empty()) {
        return match value.to_ascii_lowercase().as_str() {
            "video" => Ok("video".to_string()),
            "audio" => Ok("audio".to_string()),
            _ => Err(CliError::invalid_args(format!(
                "invalid media kind {value:?} (must be video or audio)"
            ))),
        };
    }
    match ext.trim_start_matches('.').to_ascii_lowercase().as_str() {
        "mp4" | "mov" | "avi" | "wmv" | "m4v" | "mpg" | "mpeg" | "mkv" | "webm" => {
            Ok("video".to_string())
        }
        "m4a" | "mp3" | "wav" | "aac" | "wma" | "oga" | "ogg" | "flac" => Ok("audio".to_string()),
        _ => Err(CliError::invalid_args(format!(
            "could not detect media kind from extension {ext:?}; pass --kind video|audio"
        ))),
    }
}

pub(super) fn content_type_for_media_ext(ext: &str) -> String {
    match ext.trim_start_matches('.').to_ascii_lowercase().as_str() {
        "mp4" | "m4v" => "video/mp4",
        "mov" => "video/quicktime",
        "avi" => "video/x-msvideo",
        "wmv" => "video/x-ms-wmv",
        "mpg" | "mpeg" => "video/mpeg",
        "mkv" => "video/x-matroska",
        "webm" => "video/webm",
        "m4a" => "audio/x-m4a",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "aac" => "audio/aac",
        "wma" => "audio/x-ms-wma",
        "oga" | "ogg" => "audio/ogg",
        "flac" => "audio/flac",
        _ => "application/octet-stream",
    }
    .to_string()
}

pub(super) fn poster_content_type_for_path(path: &str) -> String {
    match Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        _ => "image/png",
    }
    .to_string()
}

pub(super) fn extension_for_content_type(content_type: &str) -> &'static str {
    match content_type.to_ascii_lowercase().as_str() {
        "image/jpeg" | "image/jpg" => ".jpg",
        "image/gif" => ".gif",
        "image/bmp" => ".bmp",
        _ => ".png",
    }
}

pub(super) fn file_extension_with_dot(path: &str) -> String {
    let ext = Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default();
    if ext.is_empty() {
        String::new()
    } else {
        format!(".{ext}")
    }
}

pub(super) fn reject_media_url(path: &str) -> CliResult<()> {
    let lower = path.to_ascii_lowercase();
    if lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("ftp://")
        || lower.starts_with("rtmp://")
        || lower.starts_with("rtsp://")
    {
        return Err(CliError::invalid_args(
            "online/streaming media is not supported; --file must be a local path",
        ));
    }
    Ok(())
}
