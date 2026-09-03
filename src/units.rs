use quick_xml::Reader;
use quick_xml::events::Event;

use crate::{CliError, CliResult, attr, local_name, zip_text};

pub(crate) const EMU_PER_INCH: i64 = 914_400;
const ACCEPTED: &str = "accepted units: in, cm, mm, pt, px, %, emu, or a bare EMU number";

pub(crate) fn parse_length(raw: &str, reference: Option<i64>) -> CliResult<i64> {
    let value = raw.trim().to_ascii_lowercase();
    let (number, factor) = if let Some(number) = value.strip_suffix("emu") {
        (number, 1.0)
    } else if let Some(number) = value.strip_suffix("cm") {
        (number, EMU_PER_INCH as f64 / 2.54)
    } else if let Some(number) = value.strip_suffix("mm") {
        (number, EMU_PER_INCH as f64 / 25.4)
    } else if let Some(number) = value.strip_suffix("in") {
        (number, EMU_PER_INCH as f64)
    } else if let Some(number) = value.strip_suffix("pt") {
        (number, EMU_PER_INCH as f64 / 72.0)
    } else if let Some(number) = value.strip_suffix("px") {
        (number, EMU_PER_INCH as f64 / 96.0)
    } else if let Some(number) = value.strip_suffix('%') {
        let reference = reference.ok_or_else(|| {
            CliError::invalid_args(format!(
                "percentage length {raw:?} needs a slide or page dimension; {ACCEPTED}"
            ))
        })?;
        (number, reference as f64 / 100.0)
    } else {
        (value.as_str(), 1.0)
    };
    let number = number
        .trim()
        .parse::<f64>()
        .map_err(|_| CliError::invalid_args(format!("invalid length {raw:?}; {ACCEPTED}")))?;
    let emu = number * factor;
    if !emu.is_finite() || emu < i64::MIN as f64 || emu > i64::MAX as f64 {
        return Err(CliError::invalid_args(format!(
            "length {raw:?} is out of range; {ACCEPTED}"
        )));
    }
    Ok(emu.round() as i64)
}

pub(crate) fn inches(emu: i64) -> f64 {
    emu as f64 / EMU_PER_INCH as f64
}

pub(crate) fn presentation_slide_size(file: &str) -> CliResult<(i64, i64)> {
    let xml = zip_text(file, "ppt/presentation.xml")?;
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "sldSz" =>
            {
                let cx = attr(&e, "cx")
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(10 * EMU_PER_INCH);
                let cy = attr(&e, "cy")
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(EMU_PER_INCH * 15 / 2);
                return Ok((cx, cy));
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok((10 * EMU_PER_INCH, EMU_PER_INCH * 15 / 2))
}

fn document_page_size(file: &str) -> CliResult<(i64, i64)> {
    let xml = zip_text(file, "word/document.xml")?;
    let mut reader = Reader::from_str(&xml);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "pgSz" =>
            {
                let width_twips = attr(&e, "w")
                    .and_then(|v| v.parse::<i64>().ok())
                    .unwrap_or(12_240);
                let height_twips = attr(&e, "h")
                    .and_then(|v| v.parse::<i64>().ok())
                    .unwrap_or(15_840);
                return Ok((width_twips * 635, height_twips * 635));
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok((EMU_PER_INCH * 17 / 2, EMU_PER_INCH * 11))
}

pub(crate) fn normalize_length_args(args: &[String]) -> CliResult<Vec<String>> {
    let mut normalized = args.to_vec();
    let context = command_context(args)?;
    let Some((width, height, kind)) = context else {
        return Ok(normalized);
    };
    let flags: &[(&str, i64)] = match kind {
        ContextKind::Pptx => &[
            ("--x", width),
            ("--y", height),
            ("--cx", width),
            ("--cy", height),
        ],
        ContextKind::DocxImage => &[("--width", width), ("--height", height)],
        ContextKind::XlsxColumnWidth => &[],
    };
    for (flag, reference) in flags {
        normalize_flag(&mut normalized, flag, *reference)?;
    }
    if kind == ContextKind::Pptx {
        normalize_bounds_flag(&mut normalized, width, height)?;
    } else if kind == ContextKind::XlsxColumnWidth {
        normalize_column_width_flag(&mut normalized)?;
    }
    Ok(normalized)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ContextKind {
    Pptx,
    DocxImage,
    XlsxColumnWidth,
}

fn command_context(args: &[String]) -> CliResult<Option<(i64, i64, ContextKind)>> {
    if matches!(args.get(0..3), Some(prefix) if prefix == ["xlsx", "colwidths", "set"]) {
        return Ok(Some((0, 0, ContextKind::XlsxColumnWidth)));
    }
    let pptx_length_command = args.get(0).is_some_and(|v| v == "pptx")
        && (args.get(1).is_some_and(|v| v == "add-textbox")
            || matches!(args.get(1..3), Some(prefix) if matches!(prefix, [group, verb]
                if (group == "place" && matches!(verb.as_str(), "image" | "table" | "table-from-xlsx"))
                    || (group == "charts" && verb == "create")
                    || (group == "media" && verb == "add")
                    || (group == "shapes" && verb == "set-bounds")
                    || (group == "layouts" && matches!(verb.as_str(), "set-bounds" | "add-placeholder"))
                    || (group == "masters" && verb == "add-placeholder"))));
    if pptx_length_command {
        let file = if args.get(1).is_some_and(|arg| arg == "add-textbox") {
            args.get(2)
        } else {
            args.get(3)
        };
        if let Some(file) = file.filter(|file| !file.starts_with('-')) {
            let (width, height) = presentation_slide_size(file)?;
            return Ok(Some((width, height, ContextKind::Pptx)));
        }
    }
    if matches!(args.get(0..3), Some(prefix) if prefix == ["docx", "images", "insert"] || prefix == ["docx", "images", "replace"])
        && let Some(file) = args.get(3)
    {
        let (width, height) = document_page_size(file)?;
        return Ok(Some((width, height, ContextKind::DocxImage)));
    }
    Ok(None)
}

fn normalize_column_width_flag(args: &mut [String]) -> CliResult<()> {
    let Some(index) = args.iter().position(|arg| arg == "--width") else {
        return Ok(());
    };
    let raw = args
        .get(index + 1)
        .ok_or_else(|| CliError::invalid_args("--width requires a value"))?;
    let lower = raw.trim().to_ascii_lowercase();
    let pixels = if let Some(number) = lower.strip_suffix("cm") {
        Some(parse_number(number, raw)? * 96.0 / 2.54)
    } else if let Some(number) = lower.strip_suffix("in") {
        Some(parse_number(number, raw)? * 96.0)
    } else {
        None
    };
    if let Some(pixels) = pixels {
        // Excel's default Calibri 11 width uses a 7 px maximum digit width and 5 px padding.
        args[index + 1] = ((pixels - 5.0).max(0.0) / 7.0).to_string();
    }
    Ok(())
}

fn parse_number(number: &str, raw: &str) -> CliResult<f64> {
    number.trim().parse::<f64>().map_err(|_| {
        CliError::invalid_args(format!(
            "invalid column width {raw:?}; use character units, cm, or in"
        ))
    })
}

fn normalize_flag(args: &mut [String], flag: &str, reference: i64) -> CliResult<()> {
    let mut index = 0;
    while index < args.len() {
        if args[index] == flag {
            let raw = args
                .get(index + 1)
                .ok_or_else(|| CliError::invalid_args(format!("{flag} requires a value")))?;
            args[index + 1] = parse_length(raw, Some(reference))?.to_string();
            index += 2;
        } else {
            index += 1;
        }
    }
    Ok(())
}

fn normalize_bounds_flag(args: &mut [String], width: i64, height: i64) -> CliResult<()> {
    let Some(index) = args.iter().position(|arg| arg == "--bounds") else {
        return Ok(());
    };
    let raw = args
        .get(index + 1)
        .ok_or_else(|| CliError::invalid_args("--bounds requires x,y,cx,cy"))?;
    let parts = raw.split(',').map(str::trim).collect::<Vec<_>>();
    if parts.len() != 4 {
        return Err(CliError::invalid_args(
            "--bounds requires x,y,cx,cy; each length accepts in, cm, mm, pt, px, %, emu, or bare EMU",
        ));
    }
    let refs = [width, height, width, height];
    let values = parts
        .iter()
        .zip(refs)
        .map(|(part, reference)| parse_length(part, Some(reference)))
        .collect::<CliResult<Vec<_>>>()?;
    args[index + 1] = values
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",");
    Ok(())
}
