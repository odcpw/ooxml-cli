use serde_json::Value;

use crate::cli_dispatch::units::{parse_length, presentation_slide_size};
use crate::{CliError, CliResult, parse_string_flag, pptx_shapes_show};

#[derive(Clone, Copy)]
pub(crate) struct SlotBounds {
    pub(crate) x: i64,
    pub(crate) y: i64,
    pub(crate) cx: i64,
    pub(crate) cy: i64,
}

pub(crate) fn resolve(
    file: &str,
    slide: u32,
    args: &[String],
    aspect: Option<f64>,
) -> CliResult<Option<SlotBounds>> {
    let Some(name) = parse_string_flag(args, "--slot")? else {
        if parse_string_flag(args, "--inset")?.is_some()
            || parse_string_flag(args, "--aspect")?.is_some()
        {
            return Err(CliError::invalid_args(
                "--inset and --aspect require --slot",
            ));
        }
        return Ok(None);
    };
    if ["--x", "--y", "--cx", "--cy"]
        .iter()
        .any(|flag| args.iter().any(|arg| arg == flag))
    {
        return Err(CliError::invalid_args(
            "--slot cannot be combined with --x, --y, --cx, or --cy",
        ));
    }
    let (slide_cx, slide_cy) = presentation_slide_size(file)?;
    let shapes = pptx_shapes_show(file, slide, false, true)?;
    let body = shape_bounds(&shapes, "body").unwrap_or(SlotBounds {
        x: slide_cx / 20,
        y: slide_cy / 5,
        cx: slide_cx * 9 / 10,
        cy: slide_cy * 7 / 10,
    });
    let title = shape_bounds(&shapes, "title").unwrap_or(SlotBounds {
        x: slide_cx / 20,
        y: slide_cy / 20,
        cx: slide_cx * 9 / 10,
        cy: slide_cy / 7,
    });
    let lower = name.trim().to_ascii_lowercase();
    let mut bounds = match lower.as_str() {
        "body" => body,
        "full-bleed" => SlotBounds {
            x: 0,
            y: 0,
            cx: slide_cx,
            cy: slide_cy,
        },
        "title-area" => title,
        "left-half" => split(body, 0, 2, 0, 1),
        "right-half" => split(body, 1, 2, 0, 1),
        "top-half" => split(body, 0, 1, 0, 2),
        "bottom-half" => split(body, 0, 1, 1, 2),
        "left-third" => split(body, 0, 3, 0, 1),
        "center-third" => split(body, 1, 3, 0, 1),
        "right-third" => split(body, 2, 3, 0, 1),
        "caption" => SlotBounds {
            x: body.x,
            y: body.y + body.cy * 4 / 5,
            cx: body.cx,
            cy: body.cy / 5,
        },
        _ if lower.starts_with("grid:") => parse_grid(&lower, body)?,
        _ => {
            return Err(CliError::invalid_args(format!(
                "unknown slot {name:?}; accepted slots: body, left-half, right-half, top-half, bottom-half, left-third, center-third, right-third, grid:RxC:i, caption, full-bleed, title-area"
            )));
        }
    };
    if let Some(raw) = parse_string_flag(args, "--inset")? {
        let inset = parse_length(&raw, Some(bounds.cx.min(bounds.cy)))?;
        if inset < 0 || inset.saturating_mul(2) >= bounds.cx || inset.saturating_mul(2) >= bounds.cy
        {
            return Err(CliError::invalid_args(
                "--inset must be non-negative and smaller than half the slot",
            ));
        }
        bounds = SlotBounds {
            x: bounds.x + inset,
            y: bounds.y + inset,
            cx: bounds.cx - inset * 2,
            cy: bounds.cy - inset * 2,
        };
    }
    let aspect_mode = parse_string_flag(args, "--aspect")?
        .unwrap_or_else(|| "fill".to_string())
        .trim()
        .to_ascii_lowercase();
    match aspect_mode.as_str() {
        "fill" => {}
        "keep" => {
            if let Some(ratio) = aspect.filter(|ratio| ratio.is_finite() && *ratio > 0.0) {
                bounds = fit_aspect(bounds, ratio);
            }
        }
        other => {
            return Err(CliError::invalid_args(format!(
                "invalid --aspect {other:?}; accepted values: keep, fill"
            )));
        }
    }
    Ok(Some(bounds))
}

fn shape_bounds(show: &Value, kind: &str) -> Option<SlotBounds> {
    show.get("shapes")?.as_array()?.iter().find_map(|shape| {
        (shape.get("targetKind")?.as_str()? == kind)
            .then(|| {
                let b = shape.get("bounds")?;
                Some(SlotBounds {
                    x: b.get("x")?.as_i64()?,
                    y: b.get("y")?.as_i64()?,
                    cx: b.get("cx")?.as_i64()?,
                    cy: b.get("cy")?.as_i64()?,
                })
            })
            .flatten()
    })
}

fn split(b: SlotBounds, col: i64, cols: i64, row: i64, rows: i64) -> SlotBounds {
    let x0 = b.x + b.cx * col / cols;
    let x1 = b.x + b.cx * (col + 1) / cols;
    let y0 = b.y + b.cy * row / rows;
    let y1 = b.y + b.cy * (row + 1) / rows;
    SlotBounds {
        x: x0,
        y: y0,
        cx: x1 - x0,
        cy: y1 - y0,
    }
}

fn parse_grid(name: &str, body: SlotBounds) -> CliResult<SlotBounds> {
    let spec = name.strip_prefix("grid:").unwrap_or_default();
    let (dimensions, index) = spec
        .rsplit_once(':')
        .ok_or_else(|| CliError::invalid_args("grid slot must be grid:RxC:i (1-based i)"))?;
    let (rows, cols) = dimensions
        .split_once('x')
        .ok_or_else(|| CliError::invalid_args("grid slot must be grid:RxC:i (1-based i)"))?;
    let rows = rows
        .parse::<i64>()
        .ok()
        .filter(|v| *v > 0)
        .ok_or_else(|| CliError::invalid_args("grid rows must be positive"))?;
    let cols = cols
        .parse::<i64>()
        .ok()
        .filter(|v| *v > 0)
        .ok_or_else(|| CliError::invalid_args("grid columns must be positive"))?;
    let index = index
        .parse::<i64>()
        .ok()
        .filter(|v| *v >= 1 && *v <= rows * cols)
        .ok_or_else(|| CliError::invalid_args(format!("grid index must be 1..={}", rows * cols)))?
        - 1;
    Ok(split(body, index % cols, cols, index / cols, rows))
}

fn fit_aspect(b: SlotBounds, ratio: f64) -> SlotBounds {
    let current = b.cx as f64 / b.cy as f64;
    if current > ratio {
        let cx = (b.cy as f64 * ratio).round() as i64;
        SlotBounds {
            x: b.x + (b.cx - cx) / 2,
            cx,
            ..b
        }
    } else {
        let cy = (b.cx as f64 / ratio).round() as i64;
        SlotBounds {
            y: b.y + (b.cy - cy) / 2,
            cy,
            ..b
        }
    }
}
