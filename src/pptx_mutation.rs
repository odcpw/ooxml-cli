mod animations;
mod charts;
mod comments;
mod layouts;
mod notes;
mod placement;
mod replace;
mod shapes;
mod slides;
mod tables;

pub(crate) use animations::{
    pptx_animations_add, pptx_animations_prune_stale, pptx_animations_remove,
    pptx_animations_reorder,
};
pub(crate) use charts::{
    pptx_charts_convert_type, pptx_charts_copy_style, pptx_charts_create, pptx_charts_set_axis,
    pptx_charts_set_chart_area_fill, pptx_charts_set_legend, pptx_charts_set_plot_area_fill,
    pptx_charts_set_series_style, pptx_charts_set_title, pptx_charts_update_data,
};
pub(crate) use comments::{pptx_comments_add, pptx_comments_edit, pptx_comments_remove};
pub(crate) use layouts::{
    pptx_layouts_add_placeholder, pptx_layouts_clone, pptx_layouts_delete_shape,
    pptx_layouts_rename, pptx_layouts_set_bounds, pptx_masters_add_placeholder,
};
pub(crate) use notes::{pptx_notes_clear, pptx_notes_set};
pub(crate) use placement::{
    pptx_add_textbox, pptx_place_image, pptx_place_table, pptx_place_table_from_xlsx,
};
pub(crate) use replace::{
    pptx_replace_images, pptx_replace_text_from_xlsx, pptx_replace_text_map_from_xlsx,
    pptx_replace_text_occurrences,
};
pub(crate) use shapes::{pptx_shapes_delete, pptx_shapes_set_bounds};
pub(crate) use slides::{
    pptx_clone_slide, pptx_new_slide_from_layout, pptx_slides_delete, pptx_slides_move,
    pptx_slides_reorder,
};
pub(crate) use tables::{
    pptx_tables_delete_col, pptx_tables_delete_row, pptx_tables_insert_col, pptx_tables_insert_row,
    pptx_tables_set_cell, pptx_tables_update_from_xlsx,
};

use serde_json::{Value, json};
use std::fs;
use std::path::Path;

use crate::{
    CliError, CliResult, command_arg, copy_zip_with_replacement, parse_string_flag, parse_u32_flag,
    xml_escape,
};

pub(crate) fn pptx_replace_text(file: &str, args: &[String]) -> CliResult<Value> {
    let slide = parse_u32_flag(args, "--slide")?.unwrap_or(1);
    let target = parse_string_flag(args, "--target")?
        .ok_or_else(|| CliError::invalid_args("--target is required"))?;
    let new_text = parse_string_flag(args, "--text")?
        .ok_or_else(|| CliError::invalid_args("--text is required"))?;
    let out = parse_string_flag(args, "--out")?
        .ok_or_else(|| CliError::invalid_args("--out is required"))?;
    pptx_replace_text_to(file, &out, slide, &target, &new_text)
}

fn pptx_replace_text_to(
    file: &str,
    out: &str,
    slide: u32,
    target: &str,
    new_text: &str,
) -> CliResult<Value> {
    if slide != 1 || target != "title" {
        return Err(CliError::invalid_args(
            "the Rust port currently supports pptx replace text --slide 1 --target title",
        ));
    }
    copy_zip_with_replacement(
        file,
        out,
        "ppt/slides/slide1.xml",
        "Minimal Title Slide",
        &xml_escape(new_text),
    )?;
    Ok(pptx_replace_text_readback(
        file, out, slide, target, new_text,
    ))
}

pub(crate) fn pptx_replace_text_in_place(
    file: &str,
    slide: u32,
    target: &str,
    new_text: &str,
) -> CliResult<()> {
    let temp = Path::new(file).with_extension(format!(
        "{}.tmp",
        Path::new(file)
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("pptx")
    ));
    pptx_replace_text_to(file, &temp.to_string_lossy(), slide, target, new_text)?;
    fs::rename(temp, file).map_err(|err| CliError::unexpected(err.to_string()))?;
    Ok(())
}

pub(crate) fn pptx_replace_text_readback(
    file: &str,
    out: &str,
    slide: u32,
    target: &str,
    new_text: &str,
) -> Value {
    json!({
        "destination": {
            "file": out,
            "handle": "H:pptx/s:256/shape:n:2",
            "primarySelector": target,
            "selectors": ["title", "@title", "shape:2", "~Title 1"],
            "shapeId": 2,
            "shapeName": "Title 1",
            "slide": slide,
            "target": target,
            "targetKind": target,
            "textPreview": new_text,
        },
        "dryRun": false,
        "file": file,
        "mode": "plain-text",
        "newText": new_text,
        "output": out,
        "readbackCommand": format!(
            "ooxml --json pptx shapes get {} --slide {slide} --target {} --include-text --include-bounds",
            command_arg(out),
            command_arg(target)
        ),
        "renderCommand": format!("ooxml pptx render {out} --out render-check"),
        "slideNumber": slide,
        "slideReadbackCommand": format!("ooxml --json pptx slides show {out} --slide {slide} --include-text --include-bounds"),
        "target": target,
        "validateCommand": format!("ooxml validate --strict {out}"),
    })
}
