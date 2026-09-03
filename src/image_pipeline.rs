use std::io::Cursor;

use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::PngEncoder;
use image::imageops::FilterType;
use image::{
    DynamicImage, ExtendedColorType, GenericImageView, ImageEncoder, ImageError, ImageFormat,
    ImageReader, Limits,
};
use serde_json::{Map, Value, json};

use crate::{CliError, CliResult};

const EMU_PER_INCH: f64 = 914_400.0;
const MAX_IMAGE_DIMENSION: u32 = 20_000;
const MAX_DECODE_BYTES: u64 = 128 * 1024 * 1024;
pub(crate) const DEFAULT_MAX_DPI: f64 = 220.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ImageFit {
    Contain,
    Cover,
    Stretch,
}

impl ImageFit {
    pub(crate) fn parse(value: Option<&str>) -> CliResult<Self> {
        match value
            .unwrap_or("stretch")
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "contain" | "fit" | "keep" => Ok(Self::Contain),
            "cover" | "crop" | "fill" => Ok(Self::Cover),
            "stretch" => Ok(Self::Stretch),
            other => Err(CliError::invalid_args(format!(
                "invalid --fit {other:?} (accepted: contain, cover, stretch)"
            ))),
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Contain => "contain",
            Self::Cover => "cover",
            Self::Stretch => "stretch",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ImageBounds {
    pub(crate) x: i64,
    pub(crate) y: i64,
    pub(crate) cx: i64,
    pub(crate) cy: i64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct CropRect {
    pub(crate) left: u32,
    pub(crate) top: u32,
    pub(crate) right: u32,
    pub(crate) bottom: u32,
}

impl CropRect {
    pub(crate) fn is_empty(self) -> bool {
        self == Self::default()
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ImagePipelineOptions<'a> {
    pub(crate) placed: ImageBounds,
    pub(crate) fit: ImageFit,
    pub(crate) max_dpi: f64,
    pub(crate) keep_original: bool,
    pub(crate) alt: &'a str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ImageProbe {
    pub(crate) native_width: u32,
    pub(crate) native_height: u32,
    pub(crate) oriented_width: u32,
    pub(crate) oriented_height: u32,
    pub(crate) exif_orientation: u16,
}

#[derive(Clone, Debug)]
pub(crate) struct ProcessedImage {
    pub(crate) data: Vec<u8>,
    pub(crate) content_type: &'static str,
    pub(crate) extension: &'static str,
    pub(crate) format: &'static str,
    pub(crate) native_width: u32,
    pub(crate) native_height: u32,
    pub(crate) encoded_width: u32,
    pub(crate) encoded_height: u32,
    pub(crate) placed: ImageBounds,
    pub(crate) crop: CropRect,
    pub(crate) exif_orientation: u16,
    pub(crate) orientation_applied: bool,
    pub(crate) max_dpi: f64,
    pub(crate) keep_original: bool,
    pub(crate) fit: ImageFit,
    pub(crate) alt: String,
    pub(crate) original_bytes: usize,
}

impl ProcessedImage {
    pub(crate) fn bytes_saved(&self) -> usize {
        self.original_bytes.saturating_sub(self.data.len())
    }

    pub(crate) fn report_json(&self) -> Value {
        let mut report = Map::new();
        report.insert("imageFormat".to_string(), json!(self.format));
        report.insert("nativeWidthPx".to_string(), json!(self.native_width));
        report.insert("nativeHeightPx".to_string(), json!(self.native_height));
        report.insert("encodedWidthPx".to_string(), json!(self.encoded_width));
        report.insert("encodedHeightPx".to_string(), json!(self.encoded_height));
        report.insert("placedWidthEmu".to_string(), json!(self.placed.cx));
        report.insert("placedHeightEmu".to_string(), json!(self.placed.cy));
        report.insert(
            "placedWidthInches".to_string(),
            json!(self.placed.cx as f64 / EMU_PER_INCH),
        );
        report.insert(
            "placedHeightInches".to_string(),
            json!(self.placed.cy as f64 / EMU_PER_INCH),
        );
        report.insert("bytesOriginal".to_string(), json!(self.original_bytes));
        report.insert("bytesEmbedded".to_string(), json!(self.data.len()));
        report.insert("bytesSaved".to_string(), json!(self.bytes_saved()));
        report.insert("exifOrientation".to_string(), json!(self.exif_orientation));
        report.insert(
            "orientationApplied".to_string(),
            json!(self.orientation_applied),
        );
        report.insert("maxDpi".to_string(), json!(self.max_dpi));
        report.insert("keepOriginal".to_string(), json!(self.keep_original));
        report.insert("fit".to_string(), json!(self.fit.as_str()));
        report.insert("altText".to_string(), json!(self.alt));
        if !self.crop.is_empty() {
            report.insert(
                "crop".to_string(),
                json!({
                    "left": self.crop.left,
                    "top": self.crop.top,
                    "right": self.crop.right,
                    "bottom": self.crop.bottom,
                }),
            );
        }
        Value::Object(report)
    }
}

pub(crate) fn parse_max_dpi(value: Option<&str>) -> CliResult<f64> {
    let Some(value) = value else {
        return Ok(DEFAULT_MAX_DPI);
    };
    let parsed = value
        .trim()
        .parse::<f64>()
        .map_err(|_| CliError::invalid_args("--max-dpi must be a positive number"))?;
    if !parsed.is_finite() || parsed <= 0.0 {
        return Err(CliError::invalid_args(
            "--max-dpi must be a positive number",
        ));
    }
    Ok(parsed)
}

pub(crate) fn probe_image(bytes: &[u8]) -> CliResult<ImageProbe> {
    let detected = detect_format(bytes)?;
    if detected == DetectedFormat::Svg {
        let (width, height) = svg_dimensions(bytes)?;
        return Ok(ImageProbe {
            native_width: width,
            native_height: height,
            oriented_width: width,
            oriented_height: height,
            exif_orientation: 1,
        });
    }
    let image = decode_raster(bytes, detected.image_format().expect("raster format"))?;
    let (native_width, native_height) = image.dimensions();
    let exif_orientation = if detected == DetectedFormat::Jpeg {
        jpeg_exif_orientation(bytes).unwrap_or(1)
    } else {
        1
    };
    let (oriented_width, oriented_height) = if (5..=8).contains(&exif_orientation) {
        (native_height, native_width)
    } else {
        (native_width, native_height)
    };
    Ok(ImageProbe {
        native_width,
        native_height,
        oriented_width,
        oriented_height,
        exif_orientation,
    })
}

pub(crate) fn process_image(
    bytes: &[u8],
    options: &ImagePipelineOptions<'_>,
) -> CliResult<ProcessedImage> {
    if options.placed.cx <= 0 || options.placed.cy <= 0 {
        return Err(CliError::invalid_args(
            "placed image width and height must be positive",
        ));
    }
    if !options.max_dpi.is_finite() || options.max_dpi <= 0.0 {
        return Err(CliError::invalid_args(
            "--max-dpi must be a positive number",
        ));
    }
    let detected = detect_format(bytes)?;
    if detected == DetectedFormat::Svg {
        let (width, height) = svg_dimensions(bytes)?;
        let (placed, crop) = fit_geometry(options.placed, width, height, options.fit);
        return Ok(ProcessedImage {
            data: bytes.to_vec(),
            content_type: "image/svg+xml",
            extension: ".svg",
            format: "svg",
            native_width: width,
            native_height: height,
            encoded_width: width,
            encoded_height: height,
            placed,
            crop,
            exif_orientation: 1,
            orientation_applied: false,
            max_dpi: options.max_dpi,
            keep_original: options.keep_original,
            fit: options.fit,
            alt: options.alt.to_string(),
            original_bytes: bytes.len(),
        });
    }

    let image_format = detected.image_format().expect("raster format");
    let decoded = decode_raster(bytes, image_format)?;
    let (native_width, native_height) = decoded.dimensions();
    let exif_orientation = if detected == DetectedFormat::Jpeg {
        jpeg_exif_orientation(bytes).unwrap_or(1)
    } else {
        1
    };
    let orientation_applied = exif_orientation != 1;
    let oriented = apply_orientation(decoded, exif_orientation);
    let (oriented_width, oriented_height) = oriented.dimensions();
    let (placed, crop) = fit_geometry(options.placed, oriented_width, oriented_height, options.fit);
    let resized = downsample(oriented, placed, options.max_dpi, options.keep_original);
    let (encoded_width, encoded_height) = resized.dimensions();
    let resized_needed = encoded_width != oriented_width || encoded_height != oriented_height;
    let preserve_original = !orientation_applied && !resized_needed;
    let (data, content_type, extension, format) = if preserve_original {
        (
            bytes.to_vec(),
            detected.content_type(),
            detected.extension(),
            detected.name(),
        )
    } else if detected == DetectedFormat::Jpeg {
        (encode_jpeg(&resized)?, "image/jpeg", ".jpeg", "jpeg")
    } else {
        (encode_png(&resized)?, "image/png", ".png", "png")
    };
    Ok(ProcessedImage {
        data,
        content_type,
        extension,
        format,
        native_width,
        native_height,
        encoded_width,
        encoded_height,
        placed,
        crop,
        exif_orientation,
        orientation_applied,
        max_dpi: options.max_dpi,
        keep_original: options.keep_original,
        fit: options.fit,
        alt: options.alt.to_string(),
        original_bytes: bytes.len(),
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DetectedFormat {
    Png,
    Jpeg,
    Gif,
    Bmp,
    WebP,
    Svg,
}

impl DetectedFormat {
    fn image_format(self) -> Option<ImageFormat> {
        match self {
            Self::Png => Some(ImageFormat::Png),
            Self::Jpeg => Some(ImageFormat::Jpeg),
            Self::Gif => Some(ImageFormat::Gif),
            Self::Bmp => Some(ImageFormat::Bmp),
            Self::WebP => Some(ImageFormat::WebP),
            Self::Svg => None,
        }
    }

    fn content_type(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Gif => "image/gif",
            Self::Bmp => "image/bmp",
            Self::WebP => "image/webp",
            Self::Svg => "image/svg+xml",
        }
    }

    fn extension(self) -> &'static str {
        match self {
            Self::Png => ".png",
            Self::Jpeg => ".jpeg",
            Self::Gif => ".gif",
            Self::Bmp => ".bmp",
            Self::WebP => ".webp",
            Self::Svg => ".svg",
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpeg",
            Self::Gif => "gif",
            Self::Bmp => "bmp",
            Self::WebP => "webp",
            Self::Svg => "svg",
        }
    }
}

fn detect_format(bytes: &[u8]) -> CliResult<DetectedFormat> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Ok(DetectedFormat::Png)
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Ok(DetectedFormat::Jpeg)
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Ok(DetectedFormat::Gif)
    } else if bytes.starts_with(b"BM") {
        Ok(DetectedFormat::Bmp)
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Ok(DetectedFormat::WebP)
    } else if looks_like_svg(bytes) {
        Ok(DetectedFormat::Svg)
    } else {
        Err(CliError::unsupported_type(
            "unsupported image payload (accepted: PNG, JPEG, GIF, BMP, WebP, SVG)",
        ))
    }
}

fn decode_raster(bytes: &[u8], format: ImageFormat) -> CliResult<DynamicImage> {
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_DIMENSION);
    limits.max_image_height = Some(MAX_IMAGE_DIMENSION);
    limits.max_alloc = Some(MAX_DECODE_BYTES);
    reader.limits(limits);
    reader.decode().map_err(|err| match err {
        ImageError::Limits(_) => CliError::invalid_args(format!(
            "image exceeds decode limits (maximum {MAX_IMAGE_DIMENSION}px per axis and {MAX_DECODE_BYTES} decoded bytes): {err}"
        )),
        _ => CliError::invalid_args(format!("failed to decode image: {err}")),
    })
}

fn looks_like_svg(bytes: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    let trimmed = text.trim_start_matches(['\u{feff}', ' ', '\t', '\r', '\n']);
    trimmed.starts_with("<svg") || (trimmed.starts_with("<?xml") && trimmed.contains("<svg"))
}

fn svg_dimensions(bytes: &[u8]) -> CliResult<(u32, u32)> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| CliError::invalid_args("SVG image must be UTF-8"))?;
    let tag_start = text
        .find("<svg")
        .ok_or_else(|| CliError::invalid_args("SVG root element not found"))?;
    let tag_end = text[tag_start..]
        .find('>')
        .map(|index| tag_start + index)
        .ok_or_else(|| CliError::invalid_args("SVG root element is incomplete"))?;
    let tag = &text[tag_start..=tag_end];
    let width = svg_number_attr(tag, "width");
    let height = svg_number_attr(tag, "height");
    if let (Some(width), Some(height)) = (width, height) {
        return checked_svg_dimensions(width, height);
    }
    if let Some(view_box) = svg_attr(tag, "viewBox") {
        let values = view_box
            .split(|ch: char| ch.is_ascii_whitespace() || ch == ',')
            .filter(|value| !value.is_empty())
            .filter_map(|value| value.parse::<f64>().ok())
            .collect::<Vec<_>>();
        if values.len() == 4 && values[2] > 0.0 && values[3] > 0.0 {
            return checked_svg_dimensions(values[2].round() as u32, values[3].round() as u32);
        }
    }
    Err(CliError::invalid_args(
        "SVG must declare positive width/height or viewBox dimensions",
    ))
}

fn checked_svg_dimensions(width: u32, height: u32) -> CliResult<(u32, u32)> {
    if width > MAX_IMAGE_DIMENSION || height > MAX_IMAGE_DIMENSION {
        return Err(CliError::invalid_args(format!(
            "SVG exceeds image limits (maximum {MAX_IMAGE_DIMENSION}px per axis; received {width}x{height})"
        )));
    }
    Ok((width, height))
}

fn svg_number_attr(tag: &str, name: &str) -> Option<u32> {
    let raw = svg_attr(tag, name)?;
    let number = raw
        .trim()
        .trim_end_matches(|ch: char| ch.is_ascii_alphabetic() || ch == '%')
        .parse::<f64>()
        .ok()?;
    (number.is_finite() && number > 0.0).then(|| number.round() as u32)
}

fn svg_attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let pattern = format!("{name}=");
    let start = tag.find(&pattern)? + pattern.len();
    let quote = tag.as_bytes().get(start).copied()?;
    if quote != b'\'' && quote != b'"' {
        return None;
    }
    let value_start = start + 1;
    let value_end = tag[value_start..].find(quote as char)? + value_start;
    Some(&tag[value_start..value_end])
}

fn apply_orientation(image: DynamicImage, orientation: u16) -> DynamicImage {
    match orientation {
        2 => image.fliph(),
        3 => image.rotate180(),
        4 => image.flipv(),
        5 => image.fliph().rotate270(),
        6 => image.rotate90(),
        7 => image.fliph().rotate90(),
        8 => image.rotate270(),
        _ => image,
    }
}

fn downsample(
    image: DynamicImage,
    placed: ImageBounds,
    max_dpi: f64,
    keep_original: bool,
) -> DynamicImage {
    if keep_original {
        return image;
    }
    let max_width = ((placed.cx as f64 / EMU_PER_INCH) * max_dpi)
        .round()
        .max(1.0) as u32;
    let max_height = ((placed.cy as f64 / EMU_PER_INCH) * max_dpi)
        .round()
        .max(1.0) as u32;
    let (width, height) = image.dimensions();
    if width <= max_width && height <= max_height {
        image
    } else {
        image.resize(max_width, max_height, FilterType::Lanczos3)
    }
}

fn fit_geometry(
    requested: ImageBounds,
    width: u32,
    height: u32,
    fit: ImageFit,
) -> (ImageBounds, CropRect) {
    if width == 0 || height == 0 || fit == ImageFit::Stretch {
        return (requested, CropRect::default());
    }
    let image_aspect = width as f64 / height as f64;
    let box_aspect = requested.cx as f64 / requested.cy as f64;
    match fit {
        ImageFit::Contain => {
            if image_aspect > box_aspect {
                let cy = (requested.cx as f64 / image_aspect).round() as i64;
                (
                    ImageBounds {
                        x: requested.x,
                        y: requested.y + (requested.cy - cy) / 2,
                        cx: requested.cx,
                        cy,
                    },
                    CropRect::default(),
                )
            } else {
                let cx = (requested.cy as f64 * image_aspect).round() as i64;
                (
                    ImageBounds {
                        x: requested.x + (requested.cx - cx) / 2,
                        y: requested.y,
                        cx,
                        cy: requested.cy,
                    },
                    CropRect::default(),
                )
            }
        }
        ImageFit::Cover => {
            let crop = if image_aspect > box_aspect {
                let visible = box_aspect / image_aspect;
                let side = ((1.0 - visible) * 50_000.0).round() as u32;
                CropRect {
                    left: side,
                    right: side,
                    ..CropRect::default()
                }
            } else {
                let visible = image_aspect / box_aspect;
                let side = ((1.0 - visible) * 50_000.0).round() as u32;
                CropRect {
                    top: side,
                    bottom: side,
                    ..CropRect::default()
                }
            };
            (requested, crop)
        }
        ImageFit::Stretch => (requested, CropRect::default()),
    }
}

fn encode_jpeg(image: &DynamicImage) -> CliResult<Vec<u8>> {
    let rgb = image.to_rgb8();
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, 82)
        .encode(
            rgb.as_raw(),
            rgb.width(),
            rgb.height(),
            ExtendedColorType::Rgb8,
        )
        .map_err(|err| CliError::unexpected(format!("failed to encode JPEG: {err}")))?;
    Ok(out)
}

fn encode_png(image: &DynamicImage) -> CliResult<Vec<u8>> {
    let rgba = image.to_rgba8();
    let mut out = Vec::new();
    PngEncoder::new(Cursor::new(&mut out))
        .write_image(
            rgba.as_raw(),
            rgba.width(),
            rgba.height(),
            ExtendedColorType::Rgba8,
        )
        .map_err(|err| CliError::unexpected(format!("failed to encode PNG: {err}")))?;
    Ok(out)
}

fn jpeg_exif_orientation(bytes: &[u8]) -> Option<u16> {
    if !bytes.starts_with(&[0xff, 0xd8]) {
        return None;
    }
    let mut offset = 2;
    while offset + 4 <= bytes.len() {
        if bytes[offset] != 0xff {
            return None;
        }
        let marker = bytes[offset + 1];
        offset += 2;
        if marker == 0xd9 || marker == 0xda {
            break;
        }
        if marker == 0x01 || (0xd0..=0xd7).contains(&marker) {
            continue;
        }
        let length = u16::from_be_bytes([bytes[offset], bytes[offset + 1]]) as usize;
        if length < 2 || offset + length > bytes.len() {
            return None;
        }
        let payload = &bytes[offset + 2..offset + length];
        if marker == 0xe1 && payload.starts_with(b"Exif\0\0") {
            return tiff_orientation(&payload[6..]);
        }
        offset += length;
    }
    None
}

fn tiff_orientation(tiff: &[u8]) -> Option<u16> {
    if tiff.len() < 8 {
        return None;
    }
    let little = match &tiff[..2] {
        b"II" => true,
        b"MM" => false,
        _ => return None,
    };
    if read_u16(tiff, 2, little)? != 42 {
        return None;
    }
    let ifd = read_u32(tiff, 4, little)? as usize;
    let count = read_u16(tiff, ifd, little)? as usize;
    for index in 0..count {
        let entry = ifd.checked_add(2 + index * 12)?;
        if read_u16(tiff, entry, little)? == 0x0112
            && read_u16(tiff, entry + 2, little)? == 3
            && read_u32(tiff, entry + 4, little)? == 1
        {
            let orientation = read_u16(tiff, entry + 8, little)?;
            return (1..=8).contains(&orientation).then_some(orientation);
        }
    }
    None
}

fn read_u16(bytes: &[u8], offset: usize, little: bool) -> Option<u16> {
    let raw: [u8; 2] = bytes.get(offset..offset + 2)?.try_into().ok()?;
    Some(if little {
        u16::from_le_bytes(raw)
    } else {
        u16::from_be_bytes(raw)
    })
}

fn read_u32(bytes: &[u8], offset: usize, little: bool) -> Option<u32> {
    let raw: [u8; 4] = bytes.get(offset..offset + 4)?.try_into().ok()?;
    Some(if little {
        u32::from_le_bytes(raw)
    } else {
        u32::from_be_bytes(raw)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgba};

    fn sample_png(width: u32, height: u32) -> Vec<u8> {
        let image = DynamicImage::ImageRgba8(ImageBuffer::from_fn(width, height, |x, y| {
            Rgba([(x % 251) as u8, (y % 251) as u8, 120, ((x + y) % 255) as u8])
        }));
        encode_png(&image).expect("encode sample")
    }

    #[test]
    fn png_alpha_downsamples_deterministically() {
        let bytes = sample_png(800, 400);
        let options = ImagePipelineOptions {
            placed: ImageBounds {
                x: 0,
                y: 0,
                cx: 914_400,
                cy: 914_400,
            },
            fit: ImageFit::Contain,
            max_dpi: 220.0,
            keep_original: false,
            alt: "gradient",
        };
        let first = process_image(&bytes, &options).expect("process");
        let second = process_image(&bytes, &options).expect("process twice");
        assert_eq!(first.data, second.data);
        assert_eq!(first.content_type, "image/png");
        assert_eq!((first.encoded_width, first.encoded_height), (220, 110));
        assert_eq!(first.placed.cy, 457_200);
        let decoded = image::load_from_memory(&first.data).expect("decode result");
        assert!(decoded.color().has_alpha());
    }

    #[test]
    fn cover_geometry_crops_symmetrically_and_stretch_does_not() {
        let bounds = ImageBounds {
            x: 10,
            y: 20,
            cx: 1_000,
            cy: 1_000,
        };
        let (cover, crop) = fit_geometry(bounds, 2_000, 1_000, ImageFit::Cover);
        assert_eq!(cover, bounds);
        assert_eq!(crop.left, 25_000);
        assert_eq!(crop.right, 25_000);
        assert_eq!(
            fit_geometry(bounds, 2_000, 1_000, ImageFit::Stretch).0,
            bounds
        );
    }

    #[test]
    fn malformed_payload_and_max_dpi_have_actionable_errors() {
        let error = detect_format(b"not an image").expect_err("must reject");
        assert!(error.message.contains("PNG, JPEG, GIF, BMP, WebP, SVG"));
        let error = parse_max_dpi(Some("zero")).expect_err("must reject");
        assert_eq!(error.message, "--max-dpi must be a positive number");
    }

    #[test]
    fn oversized_image_fails_before_decode_allocation() {
        let bytes = sample_png(MAX_IMAGE_DIMENSION + 1, 1);
        let options = ImagePipelineOptions {
            placed: ImageBounds {
                x: 0,
                y: 0,
                cx: 914_400,
                cy: 914_400,
            },
            fit: ImageFit::Contain,
            max_dpi: DEFAULT_MAX_DPI,
            keep_original: false,
            alt: "oversized",
        };
        let error = process_image(&bytes, &options).expect_err("must reject oversized input");
        assert!(error.message.contains("image exceeds decode limits"));
        assert!(error.message.contains("20000px"));
    }
}
