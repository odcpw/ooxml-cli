use std::path::Path;

use crate::{CliError, CliResult};

pub(super) fn normalize_fit_mode(mode: &str) -> CliResult<String> {
    match mode.to_ascii_lowercase().as_str() {
        "contain" | "fit" => Ok("contain".to_string()),
        "cover" | "crop" => Ok("cover".to_string()),
        other => Err(CliError::invalid_args(format!(
            "invalid fit mode {other:?} (must be 'contain' or 'cover')"
        ))),
    }
}

pub(super) fn replacement_image_uri(
    old_uri: &str,
    old_content_type: &str,
    new_content_type: &str,
) -> CliResult<String> {
    if normalized_image_content_type(old_content_type)
        == normalized_image_content_type(new_content_type)
    {
        return Ok(old_uri.to_string());
    }
    let new_ext = image_extension_for_content_type(new_content_type)?;
    let old_ext = Path::new(old_uri)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default();
    if old_ext.eq_ignore_ascii_case(new_ext.trim_start_matches('.')) {
        return Ok(old_uri.to_string());
    }
    let Some((base, _)) = old_uri.rsplit_once('.') else {
        return Ok(format!("{old_uri}{new_ext}"));
    };
    Ok(format!("{base}{new_ext}"))
}

pub(super) fn image_content_type_from_path(path: &str) -> CliResult<String> {
    let extension = Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match extension.as_str() {
        "png" => Ok("image/png".to_string()),
        "jpg" | "jpeg" => Ok("image/jpeg".to_string()),
        "gif" => Ok("image/gif".to_string()),
        "bmp" => Ok("image/bmp".to_string()),
        "tif" | "tiff" => Ok("image/tiff".to_string()),
        "webp" => Ok("image/webp".to_string()),
        "svg" => Ok("image/svg+xml".to_string()),
        _ => Err(CliError::unsupported_type(format!(
            "unsupported image type for {path}; supported extensions are .png, .jpg, .jpeg, .gif, .bmp, .tif, .tiff, .webp, and .svg"
        ))),
    }
}

fn image_extension_for_content_type(content_type: &str) -> CliResult<&'static str> {
    match normalized_image_content_type(content_type).as_str() {
        "image/png" => Ok(".png"),
        "image/jpeg" | "image/jpg" | "image/pjpeg" => Ok(".jpg"),
        "image/gif" => Ok(".gif"),
        "image/bmp" => Ok(".bmp"),
        "image/tiff" => Ok(".tiff"),
        "image/webp" => Ok(".webp"),
        "image/svg+xml" => Ok(".svg"),
        other => Err(CliError::unsupported_type(format!(
            "unsupported image content type {other}"
        ))),
    }
}

pub(super) fn validate_image_payload(raw: &[u8], content_type: &str) -> Result<(), String> {
    let normalized = normalized_image_content_type(content_type);
    let ok = match normalized.as_str() {
        "image/png" => raw.starts_with(b"\x89PNG\r\n\x1a\n"),
        "image/jpeg" | "image/jpg" | "image/pjpeg" => {
            raw.len() >= 3 && raw[0] == 0xff && raw[1] == 0xd8 && raw[2] == 0xff
        }
        "image/gif" => raw.starts_with(b"GIF87a") || raw.starts_with(b"GIF89a"),
        "image/bmp" => raw.starts_with(b"BM"),
        "image/tiff" => raw.starts_with(b"II*\0") || raw.starts_with(b"MM\0*"),
        _ => true,
    };
    if ok {
        Ok(())
    } else {
        Err(format!(
            "image payload does not match content type {normalized}"
        ))
    }
}

fn normalized_image_content_type(content_type: &str) -> String {
    content_type
        .split_once(';')
        .map(|(head, _)| head)
        .unwrap_or(content_type)
        .trim()
        .to_ascii_lowercase()
}
