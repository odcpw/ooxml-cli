mod docx;
mod xlsx;

use serde_json::{Value, json};

use crate::capabilities;
use crate::cli_args::*;
use crate::cli_core::{CliError, CliResult, EXIT_SUCCESS, GlobalFlags};
use crate::inspect::inspect;
use crate::pptx_mutation::*;
use crate::pptx_readback::*;
use crate::pptx_render::pptx_render;
use crate::validation::validate;
use crate::vba::*;
use crate::verify::verify;

pub(crate) struct DispatchOutput {
    pub(crate) value: Value,
    pub(crate) exit_code: i32,
}

pub(crate) fn dispatch(flags: &GlobalFlags, args: &[String]) -> CliResult<DispatchOutput> {
    if let [family, verb, file, rest @ ..] = args
        && family == "vba"
        && verb == "office-check"
    {
        reject_unknown_flags(rest, &["--out-dir"], &[])?;
        let out_dir = parse_string_flag(rest, "--out-dir")?;
        let (value, exit_code) = vba_office_check(file, out_dir.as_deref())?;
        return Ok(DispatchOutput { value, exit_code });
    }
    dispatch_value(flags, args).map(|value| DispatchOutput {
        value,
        exit_code: EXIT_SUCCESS,
    })
}

fn dispatch_value(flags: &GlobalFlags, args: &[String]) -> CliResult<Value> {
    match args {
        [cmd] if cmd == "version" => Ok(json!({"tool": "ooxml", "version": "0.0.1"})),
        [cmd, rest @ ..] if cmd == "capabilities" => capabilities::capabilities(rest),
        [cmd, file] if cmd == "inspect" => inspect(file),
        [cmd, rest @ ..] if cmd == "validate" => {
            let (file, strict) = parse_validate_args(rest, flags.strict)?;
            validate(file, strict)
        }
        [cmd, file, rest @ ..] if cmd == "verify" => verify(file, rest),
        [family, verb, file] if family == "vba" && verb == "inspect" => vba_inspect(file),
        [family, verb, output, rest @ ..] if family == "vba" && verb == "create" => {
            reject_unknown_flags(
                rest,
                &[
                    "--family",
                    "--source",
                    "--extract-bin",
                    "--office-create-script",
                ],
                &["--enable-vba-object-model-access", "--visible", "--force"],
            )?;
            let family = parse_string_flag(rest, "--family")?;
            let sources = parse_string_flags(rest, "--source")?;
            let extract_bin = parse_string_flag(rest, "--extract-bin")?;
            let office_create_script = parse_string_flag(rest, "--office-create-script")?;
            vba_create(
                output,
                VbaCreateOptions {
                    family: family.as_deref(),
                    sources,
                    extract_bin: extract_bin.as_deref(),
                    office_create_script: office_create_script.as_deref(),
                    enable_vba_object_model_access: has_flag(
                        rest,
                        "--enable-vba-object-model-access",
                    ),
                    visible: has_flag(rest, "--visible"),
                    force: has_flag(rest, "--force"),
                },
            )
        }
        [family, verb, bin_path, rest @ ..] if family == "vba" && verb == "inspect-bin" => {
            reject_unknown_flags(rest, &["--family"], &[])?;
            let family = parse_string_flag(rest, "--family")?.ok_or_else(|| {
                CliError::invalid_args("--family is required for inspect-bin (pptx or xlsx)")
            })?;
            vba_inspect_bin(bin_path, &family)
        }
        [family, verb, file, rest @ ..] if family == "vba" && verb == "list" => {
            reject_unknown_flags(rest, &[], &[])?;
            vba_list(file)
        }
        [family, verb, file, rest @ ..] if family == "vba" && verb == "extract" => {
            reject_unknown_flags(rest, &["--out-dir", "--module"], &[])?;
            let out_dir = parse_string_flag(rest, "--out-dir")?
                .ok_or_else(|| CliError::invalid_args("--out-dir is required"))?;
            let selector = parse_string_flag(rest, "--module")?;
            vba_extract(file, &out_dir, selector.as_deref())
        }
        [family, verb, file, rest @ ..] if family == "vba" && verb == "extract-bin" => {
            reject_unknown_flags(rest, &["--out"], &[])?;
            let out = parse_string_flag(rest, "--out")?
                .ok_or_else(|| CliError::invalid_args("--out is required"))?;
            vba_extract_bin(file, &out)
        }
        [family, verb, file, rest @ ..] if family == "vba" && verb == "attach" => {
            reject_unknown_flags(
                rest,
                &["--bin", "--out", "--backup"],
                &[
                    "--allow-host-family-risk",
                    "--dry-run",
                    "--no-validate",
                    "--in-place",
                ],
            )?;
            let bin = parse_string_flag(rest, "--bin")?
                .ok_or_else(|| CliError::invalid_args("--bin is required"))?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            vba_attach(
                file,
                &bin,
                VbaMutationOptions {
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, verb, file, rest @ ..] if family == "vba" && verb == "remove" => {
            reject_unknown_flags(
                rest,
                &["--out", "--backup"],
                &["--dry-run", "--no-validate", "--in-place"],
            )?;
            let out = parse_string_flag(rest, "--out")?;
            let backup = parse_string_flag(rest, "--backup")?;
            vba_remove(
                file,
                VbaMutationOptions {
                    out: out.as_deref(),
                    backup: backup.as_deref(),
                    dry_run: has_flag(rest, "--dry-run"),
                    no_validate: has_flag(rest, "--no-validate"),
                    in_place: has_flag(rest, "--in-place"),
                },
            )
        }
        [family, ..] if family == "docx" => docx::dispatch_docx(args),
        [family, ..] if family == "xlsx" => xlsx::dispatch_xlsx(args),
        [family, verb, file, rest @ ..] if family == "pptx" && verb == "render" => {
            pptx_render(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "slides" && verb == "show" =>
        {
            let slide = parse_u32_flag(rest, "--slide")?.unwrap_or(1);
            pptx_slide_show(file, slide)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "slides" && verb == "selectors" =>
        {
            let slide = parse_u32_flag(rest, "--slide")?
                .ok_or_else(|| CliError::invalid_args("--slide is required"))?;
            pptx_slide_selectors(file, slide)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "shapes" && verb == "show" =>
        {
            reject_unknown_flags(rest, &["--slide"], &["--include-text", "--include-bounds"])?;
            let slide = parse_u32_flag(rest, "--slide")?
                .ok_or_else(|| CliError::invalid_args("required flag(s) \"slide\" not set"))?;
            let include_text = has_flag(rest, "--include-text");
            let include_bounds = has_flag(rest, "--include-bounds");
            pptx_shapes_show(file, slide, include_text, include_bounds)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "shapes" && verb == "get" =>
        {
            reject_unknown_flags(
                rest,
                &["--slide", "--target"],
                &["--include-text", "--include-bounds"],
            )?;
            let slide = parse_u32_flag(rest, "--slide")?
                .ok_or_else(|| CliError::invalid_args("required flag(s) \"slide\" not set"))?;
            let target = parse_string_flag(rest, "--target")?
                .ok_or_else(|| CliError::invalid_args("required flag(s) \"target\" not set"))?;
            let include_text = has_flag(rest, "--include-text");
            let include_bounds = has_flag(rest, "--include-bounds");
            pptx_shapes_get(file, slide, &target, include_text, include_bounds)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "shapes" && verb == "set-bounds" =>
        {
            reject_unknown_flags(
                rest,
                &["--slide", "--target", "--bounds", "--out", "--backup"],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_shapes_set_bounds(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "shapes" && verb == "delete" =>
        {
            reject_unknown_flags(
                rest,
                &["--slide", "--target", "--out", "--backup"],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_shapes_delete(file, rest)
        }
        [family, group, verb, file] if family == "pptx" && group == "slides" && verb == "list" => {
            pptx_slides_list(file)
        }
        [family, group, verb, file, slide, rest @ ..]
            if family == "pptx" && group == "slides" && verb == "delete" =>
        {
            reject_unknown_flags(
                rest,
                &["--out", "--backup"],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            let slide = parse_pptx_slide_lifecycle_position(slide, "slide number")?;
            pptx_slides_delete(file, slide, rest)
        }
        [
            family,
            group,
            verb,
            file,
            from_position,
            to_position,
            rest @ ..,
        ] if family == "pptx" && group == "slides" && verb == "move" => {
            reject_unknown_flags(
                rest,
                &["--out", "--backup"],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            let from_position =
                parse_pptx_slide_lifecycle_position(from_position, "from-position")?;
            let to_position = parse_pptx_slide_lifecycle_position(to_position, "to-position")?;
            pptx_slides_move(file, from_position, to_position, rest)
        }
        [family, group, verb, file, order, rest @ ..]
            if family == "pptx" && group == "slides" && verb == "reorder" =>
        {
            reject_unknown_flags(
                rest,
                &["--out", "--backup"],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_slides_reorder(file, order, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "extract" && verb == "text" =>
        {
            reject_unknown_flags(rest, &["--slide"], &[])?;
            pptx_extract_text(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "extract" && verb == "notes" =>
        {
            reject_unknown_flags(rest, &["--slide"], &[])?;
            pptx_extract_notes(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "extract" && verb == "images" =>
        {
            reject_unknown_flags(rest, &["--out", "--slide"], &["--include-layout-images"])?;
            pptx_extract_images(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "extract" && verb == "xml" =>
        {
            reject_unknown_flags(rest, &["--slide", "--layout", "--master", "--out"], &[])?;
            pptx_extract_xml(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "notes" && verb == "show" =>
        {
            reject_unknown_flags(rest, &["--slide"], &[])?;
            let slide = parse_u32_flag(rest, "--slide")?
                .ok_or_else(|| CliError::invalid_args("--slide is required"))?;
            pptx_notes_show(file, slide)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "notes" && verb == "set" =>
        {
            reject_unknown_flags(
                rest,
                &["--slide", "--text", "--out", "--backup"],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_notes_set(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "notes" && verb == "clear" =>
        {
            reject_unknown_flags(
                rest,
                &["--slide", "--out", "--backup"],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_notes_clear(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "comments" && verb == "list" =>
        {
            reject_unknown_flags(rest, &["--slide", "--comment-id"], &[])?;
            let slide = parse_i64_flag(rest, "--slide")?;
            let comment_id = parse_i64_flag(rest, "--comment-id")?;
            if let Some(slide) = slide
                && slide < 1
            {
                return Err(CliError::invalid_args("--slide must be >= 1"));
            }
            if comment_id.is_some() && slide.is_none() {
                return Err(CliError::invalid_args("--comment-id requires --slide"));
            }
            pptx_comments_list(file, slide.map(|value| value as u32), comment_id)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "comments" && verb == "add" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--slide",
                    "--author",
                    "--initials",
                    "--date",
                    "--text",
                    "--text-file",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_comments_add(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "comments" && verb == "edit" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--slide",
                    "--comment-id",
                    "--author-id",
                    "--handle",
                    "--text",
                    "--text-file",
                    "--author",
                    "--date",
                    "--expect-hash",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_comments_edit(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "comments" && verb == "remove" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--slide",
                    "--comment-id",
                    "--author-id",
                    "--handle",
                    "--expect-hash",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_comments_remove(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "masters" && verb == "list" =>
        {
            reject_unknown_flags(rest, &[], &[])?;
            pptx_masters_list(file)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "masters" && verb == "show" =>
        {
            reject_unknown_flags(rest, &["--master"], &[])?;
            let master = parse_i64_flag(rest, "--master")?.unwrap_or(1);
            pptx_masters_show(file, master)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "layouts" && verb == "list" =>
        {
            reject_unknown_flags(rest, &["--master"], &[])?;
            let master = parse_i64_flag(rest, "--master")?;
            if let Some(master) = master
                && master < 0
            {
                return Err(CliError::invalid_args("--master must be >= 0"));
            }
            pptx_layouts_list(file, master.map(|value| value as u32))
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "layouts" && verb == "show" =>
        {
            reject_unknown_flags(rest, &["--layout"], &[])?;
            let layout = parse_string_flag(rest, "--layout")?
                .ok_or_else(|| CliError::invalid_args("--layout flag is required"))?;
            pptx_layouts_show(file, &layout)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "layouts" && verb == "rename" =>
        {
            reject_unknown_flags(
                rest,
                &["--layout", "--name", "--out", "--backup"],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_layouts_rename(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "layouts" && verb == "set-bounds" =>
        {
            reject_unknown_flags(
                rest,
                &["--layout", "--target", "--bounds", "--out", "--backup"],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_layouts_set_bounds(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "layouts" && verb == "delete-shape" =>
        {
            reject_unknown_flags(
                rest,
                &["--layout", "--target", "--out", "--backup"],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_layouts_delete_shape(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "layouts" && verb == "add-placeholder" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--layout", "--type", "--bounds", "--idx", "--size", "--orient", "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_layouts_add_placeholder(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "charts" && verb == "list" =>
        {
            reject_unknown_flags(rest, &["--slide"], &[])?;
            let slide = parse_i64_flag(rest, "--slide")?.unwrap_or(0);
            pptx_charts_list(file, slide)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "charts" && verb == "show" =>
        {
            reject_unknown_flags(rest, &["--slide", "--chart"], &[])?;
            let slide = parse_i64_flag(rest, "--slide")?.unwrap_or(0);
            let chart = parse_string_flag(rest, "--chart")?;
            pptx_charts_show(file, slide, chart.as_deref())
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "charts" && verb == "set-title" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--slide",
                    "--chart",
                    "--title",
                    "--expect-title",
                    "--font-family",
                    "--font-size",
                    "--font-color",
                    "--out",
                    "--backup",
                ],
                &[
                    "--font-bold",
                    "--font-italic",
                    "--dry-run",
                    "--in-place",
                    "--no-validate",
                ],
            )?;
            pptx_charts_set_title(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "charts" && verb == "set-legend" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--slide",
                    "--chart",
                    "--position",
                    "--overlay",
                    "--expect-position",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_charts_set_legend(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "charts" && verb == "set-chart-area-fill" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--slide",
                    "--chart",
                    "--fill-color",
                    "--expect-fill",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_charts_set_chart_area_fill(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "charts" && verb == "set-plot-area-fill" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--slide",
                    "--chart",
                    "--fill-color",
                    "--expect-fill",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_charts_set_plot_area_fill(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "charts" && verb == "set-series-style" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--slide",
                    "--chart",
                    "--series",
                    "--fill-color",
                    "--line-color",
                    "--line-width-pt",
                    "--marker-symbol",
                    "--marker-size",
                    "--expect-series-count",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_charts_set_series_style(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "tables" && verb == "show" =>
        {
            reject_unknown_flags(rest, &["--slide", "--table-id", "--target"], &["--details"])?;
            let slide = parse_i64_flag(rest, "--slide")?.unwrap_or(0);
            if slide < 1 {
                return Err(CliError::invalid_args("--slide must be >= 1"));
            }
            let table_id = parse_i64_flag(rest, "--table-id")?.unwrap_or(0);
            if table_id < 0 {
                return Err(CliError::invalid_args(
                    "--table-id must be a positive integer",
                ));
            }
            let target = parse_string_flag(rest, "--target")?;
            if table_id > 0 && target.as_deref().unwrap_or_default() != "" {
                return Err(CliError::invalid_args(
                    "specify only one of --target or --table-id",
                ));
            }
            pptx_tables_show(
                file,
                slide as u32,
                table_id as u32,
                target.as_deref(),
                has_flag(rest, "--details"),
            )
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "tables" && verb == "delete-row" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--slide",
                    "--table-id",
                    "--target",
                    "--row",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_tables_delete_row(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "tables" && verb == "insert-row" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--slide",
                    "--table-id",
                    "--target",
                    "--at",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_tables_insert_row(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "tables" && verb == "delete-col" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--slide",
                    "--table-id",
                    "--target",
                    "--col",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_tables_delete_col(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "tables" && verb == "insert-col" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--slide",
                    "--table-id",
                    "--target",
                    "--at",
                    "--width-emu",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_tables_insert_col(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "tables" && verb == "set-cell" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--slide",
                    "--table-id",
                    "--target",
                    "--row",
                    "--col",
                    "--text",
                    "--text-file",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_tables_set_cell(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "tables" && verb == "update-from-xlsx" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--slide",
                    "--table-id",
                    "--target",
                    "--workbook",
                    "--sheet",
                    "--range",
                    "--table",
                    "--max-cells",
                    "--formula-mode",
                    "--expect-source-range",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_tables_update_from_xlsx(file, rest)
        }
        [family, group, verb]
            if family == "pptx"
                && group == "replace"
                && (verb.as_str() == "text-from-xlsx" || verb.as_str() == "text-map-from-xlsx") =>
        {
            Err(CliError::invalid_args("accepts 1 arg(s), received 0"))
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "replace" && verb == "text" =>
        {
            reject_unknown_flags(rest, &["--slide", "--target", "--text", "--out"], &[])?;
            pptx_replace_text(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "replace" && verb == "text-from-xlsx" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--slide",
                    "--target",
                    "--workbook",
                    "--sheet",
                    "--range",
                    "--max-cells",
                    "--formula-mode",
                    "--mode",
                    "--row-sep",
                    "--col-sep",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_replace_text_from_xlsx(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "replace" && verb == "text-map-from-xlsx" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--workbook",
                    "--sheet",
                    "--range",
                    "--table",
                    "--max-cells",
                    "--formula-mode",
                    "--mode",
                    "--slide-col",
                    "--target-col",
                    "--text-col",
                    "--expect-source-range",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_replace_text_map_from_xlsx(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "replace" && verb == "text-occurrences" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--match-text",
                    "--new-text",
                    "--new-text-file",
                    "--for-slides",
                    "--for-shape",
                    "--expect-count",
                    "--expect-plan-hash",
                    "--out",
                    "--backup",
                ],
                &[
                    "--ignore-case",
                    "--allow-zero",
                    "--dry-run",
                    "--in-place",
                    "--no-validate",
                ],
            )?;
            pptx_replace_text_occurrences(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "replace" && verb == "images" =>
        {
            reject_unknown_flags(
                rest,
                &[
                    "--target",
                    "--image",
                    "--fit-mode",
                    "--slide",
                    "--for-slides",
                    "--out",
                    "--backup",
                ],
                &["--dry-run", "--in-place", "--no-validate"],
            )?;
            pptx_replace_images(file, rest)
        }
        _ => Err(CliError::invalid_args(format!(
            "unsupported Rust-port contract command: {}",
            args.join(" ")
        ))),
    }
}

fn parse_pptx_slide_lifecycle_position(value: &str, label: &str) -> CliResult<i64> {
    value.parse::<i64>().map_err(|_| {
        CliError::invalid_args(format!("invalid {label}: {value} (expected an integer)"))
    })
}

pub(crate) fn require_docx_block_hash(value: &str) -> CliResult<()> {
    if value.trim().is_empty() {
        return Err(CliError::invalid_args("--expect-hash is required"));
    }
    let Some(hex) = value.strip_prefix("sha256:") else {
        return Err(CliError::invalid_args(
            "--expect-hash must match sha256:<64 lowercase hex chars> from docx blocks",
        ));
    };
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|ch| ch.is_ascii_hexdigit() && !ch.is_ascii_uppercase())
    {
        return Err(CliError::invalid_args(
            "--expect-hash must match sha256:<64 lowercase hex chars> from docx blocks",
        ));
    }
    Ok(())
}
